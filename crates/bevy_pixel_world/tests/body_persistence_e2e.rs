//! E2E test for pixel body persistence across chunk save/load cycles.
//!
//! Tests that pixel bodies survive chunk unload + reload without becoming
//! "dead" (empty written_positions, unable to blit).
//!
//! Run: cargo test -p bevy_pixel_world body_persistence_e2e

use std::path::Path;

use bevy::app::{TaskPoolOptions, TaskPoolPlugin};
use bevy::prelude::*;
use bevy_pixel_world::{
  AsyncTaskBehavior, CHUNK_SIZE, ColorIndex, DisplacementState, LastBlitTransform, MaterialSeeder,
  Persistable, PersistenceConfig, Pixel, PixelBodiesPlugin, PixelBody, PixelBodyId,
  PixelBodyIdGenerator, PixelWorld, PixelWorldPlugin, SpawnPixelWorld, StreamingCamera, WorldPos,
  material_ids,
};
use tempfile::TempDir;

const CAMERA_SPEED: f32 = 500.0;
const DELTA_TIME: f32 = 1.0 / 60.0;

struct TestHarness {
  app: App,
  camera: Entity,
}

impl TestHarness {
  fn new(save_path: &Path) -> Self {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins.set(TaskPoolPlugin {
      task_pool_options: TaskPoolOptions::with_num_threads(4),
    }));

    app.add_plugins(bevy::transform::TransformPlugin);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::image::ImagePlugin::default());
    app.add_plugins(bevy::scene::ScenePlugin);
    app.add_plugins(bevy::gizmos::GizmoPlugin);

    app.add_plugins(PixelWorldPlugin::new(PersistenceConfig::at(save_path)));
    app.add_plugins(PixelBodiesPlugin);
    app.insert_resource(AsyncTaskBehavior::Poll);

    let camera = app
      .world_mut()
      .spawn((
        Transform::default(),
        GlobalTransform::default(),
        StreamingCamera,
      ))
      .id();

    app
      .world_mut()
      .commands()
      .queue(SpawnPixelWorld::new(MaterialSeeder::new(42)));

    app.update();

    Self { app, camera }
  }

  fn run_until_seeded(&mut self) {
    for i in 0..100 {
      self.app.update();
      if i % 20 == 19 {
        let mut q = self.app.world_mut().query::<&PixelWorld>();
        if let Ok(world) = q.single(self.app.world()) {
          if world.get_pixel(WorldPos::new(0, 0)).is_some() {
            return;
          }
        }
      }
    }
  }

  fn run(&mut self, updates: usize) {
    for _ in 0..updates {
      self.app.update();
    }
  }

  fn move_camera(&mut self, position: Vec3) {
    let mut transform = self
      .app
      .world_mut()
      .get_mut::<Transform>(self.camera)
      .unwrap();
    transform.translation = position;
    drop(transform);
    let mut global = self
      .app
      .world_mut()
      .get_mut::<GlobalTransform>(self.camera)
      .unwrap();
    *global = GlobalTransform::from(Transform::from_translation(position));
  }

  fn camera_position(&self) -> Vec3 {
    self
      .app
      .world()
      .get::<Transform>(self.camera)
      .unwrap()
      .translation
  }

  fn scroll_to(&mut self, target: Vec3) {
    let speed = CAMERA_SPEED * DELTA_TIME;

    loop {
      let current = self.camera_position();
      let delta = target - current;

      if delta.length() < speed {
        self.move_camera(target);
        self.app.update();
        break;
      }

      let direction = delta.normalize();
      let new_pos = current + direction * speed;
      self.move_camera(new_pos);
      self.app.update();
    }
  }

  fn spawn_pixel_body(
    &mut self,
    position: Vec2,
    size: u32,
    material: bevy_pixel_world::MaterialId,
  ) -> Entity {
    let mut body = PixelBody::new(size, size);

    for y in 0..size {
      for x in 0..size {
        body.set_pixel(x, y, Pixel::new(material, ColorIndex(100)));
      }
    }

    let body_id = {
      let mut id_gen = self.app.world_mut().resource_mut::<PixelBodyIdGenerator>();
      id_gen.generate()
    };

    let transform = Transform::from_translation(position.extend(0.0));
    let global_transform = GlobalTransform::from(transform);

    self
      .app
      .world_mut()
      .spawn((
        body,
        LastBlitTransform::default(),
        DisplacementState::default(),
        transform,
        global_transform,
        body_id,
        Persistable,
      ))
      .id()
  }

  fn body_solid_count(&self, entity: Entity) -> Option<usize> {
    self
      .app
      .world()
      .get::<PixelBody>(entity)
      .map(|b| b.solid_count())
  }

  fn count_pixel_bodies(&mut self) -> usize {
    let mut q = self.app.world_mut().query::<&PixelBody>();
    q.iter(self.app.world()).count()
  }

  fn body_written_positions_count(&self, entity: Entity) -> usize {
    self
      .app
      .world()
      .get::<LastBlitTransform>(entity)
      .map(|lbt| lbt.written_positions.len())
      .unwrap_or(0)
  }

  fn body_position(&self, entity: Entity) -> Option<Vec2> {
    self
      .app
      .world()
      .get::<Transform>(entity)
      .map(|t| t.translation.truncate())
  }
}

/// Test that pixel bodies survive chunk unload/reload cycles.
///
/// This reproduces the bug where PIXEL_BODY flagged pixels get baked into
/// saved chunk data. On reload, the body can't blit (positions already
/// flagged), leaving it with empty written_positions — effectively dead.
#[test]
fn pixel_bodies_survive_chunk_reload() {
  let temp_dir = TempDir::new().unwrap();
  let save_path = temp_dir.path().join("body_persist.save");

  let mut harness = TestHarness::new(&save_path);
  harness.move_camera(Vec3::ZERO);
  harness.run_until_seeded();

  // Spawn 3 pixel bodies at known positions
  let body_size = 8u32;
  let expected_solid = (body_size * body_size) as usize;
  let positions = [
    Vec2::new(0.0, 50.0),
    Vec2::new(50.0, 50.0),
    Vec2::new(-50.0, 50.0),
  ];

  let bodies: Vec<Entity> = positions
    .iter()
    .map(|&pos| harness.spawn_pixel_body(pos, body_size, material_ids::STONE))
    .collect();

  // Let bodies blit and settle - wait until all bodies have written_positions
  let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
  loop {
    harness.run(1);
    let all_blitted = bodies
      .iter()
      .all(|&e| harness.body_written_positions_count(e) > 0);
    if all_blitted {
      break;
    }
    if std::time::Instant::now() > deadline {
      panic!("Bodies did not blit within 5 seconds");
    }
    std::thread::yield_now();
  }

  // Record pre-scroll state
  let pre_scroll: Vec<(u64, usize, Vec2)> = bodies
    .iter()
    .map(|&e| {
      let id = harness.app.world().get::<PixelBodyId>(e).unwrap().0;
      let solid = harness.body_solid_count(e).unwrap();
      let pos = harness.body_position(e).unwrap();
      (id, solid, pos)
    })
    .collect();

  // Verify bodies are blitting (written_positions non-empty)
  for &e in &bodies {
    assert!(
      harness.body_written_positions_count(e) > 0,
      "Body should have written_positions before scroll"
    );
  }

  // Scroll far away to trigger chunk unload + body save
  let far_away = Vec3::new(5.0 * CHUNK_SIZE as f32, 0.0, 0.0);
  harness.scroll_to(far_away);
  harness.run(30);

  // All bodies should be despawned (chunks unloaded)
  assert_eq!(
    harness.count_pixel_bodies(),
    0,
    "All bodies should be despawned after scrolling away"
  );

  // Scroll back to trigger chunk reload + body load
  harness.scroll_to(Vec3::ZERO);
  harness.run(60);

  // Liveness assertions
  let body_count = harness.count_pixel_bodies();
  assert_eq!(
    body_count, 3,
    "Expected 3 pixel bodies after reload, got {}",
    body_count
  );

  // Verify each reloaded body is alive
  let mut q = harness
    .app
    .world_mut()
    .query::<(Entity, &PixelBody, &PixelBodyId, &LastBlitTransform)>();
  let reloaded: Vec<_> = q
    .iter(harness.app.world())
    .map(|(e, body, id, lbt)| (e, body.solid_count(), id.0, lbt.written_positions.len()))
    .collect();

  for (entity, solid, id, written) in &reloaded {
    // Solid count matches expected
    assert_eq!(
      *solid, expected_solid,
      "Body {} should have {} solid pixels, got {}",
      id, expected_solid, solid
    );

    // written_positions is non-empty (proves blit succeeded)
    assert!(
      *written > 0,
      "Body {} (entity {:?}) has empty written_positions — blit failed after reload",
      id,
      entity
    );
  }

  // Verify all original body IDs are present
  let reloaded_ids: Vec<u64> = reloaded.iter().map(|(_, _, id, _)| *id).collect();
  for (orig_id, orig_solid, _) in &pre_scroll {
    assert!(
      reloaded_ids.contains(orig_id),
      "Original body id {} not found after reload",
      orig_id
    );
    // Find matching reloaded body and verify solid count
    let matching = reloaded.iter().find(|(_, _, id, _)| id == orig_id).unwrap();
    assert_eq!(
      matching.1, *orig_solid,
      "Body {} solid count changed: {} -> {}",
      orig_id, orig_solid, matching.1
    );
  }
}
