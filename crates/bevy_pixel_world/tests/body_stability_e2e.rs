//! E2E test for pixel body stability and erasure.
//!
//! Tests two critical invariants:
//! 1. Bodies must not spontaneously lose pixels (disintegrate)
//! 2. Erased bodies must be fully removable (no ghosts)
//!
//! Run: cargo test -p bevy_pixel_world body_stability --features headless
//! --no-default-features

use std::path::Path;

use bevy::app::{TaskPoolOptions, TaskPoolPlugin};
use bevy::ecs::world::Mut;
use bevy::prelude::*;
use bevy_pixel_world::{
  ColorIndex, DisplacementState, LastBlitTransform, MaterialSeeder, Persistable, PersistenceConfig,
  Pixel, PixelBody, PixelBodyId, PixelBodyIdGenerator, PixelWorld, PixelWorldPlugin,
  SpawnPixelWorld, StreamingCamera, WorldPos, WorldRect, material_ids,
};
use tempfile::TempDir;

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

    // TransformPlugin is needed for GlobalTransform propagation
    app.add_plugins(bevy::transform::TransformPlugin);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::image::ImagePlugin::default());
    app.add_plugins(bevy::scene::ScenePlugin);
    app.add_plugins(bevy::gizmos::GizmoPlugin);

    app.add_plugins(
      PixelWorldPlugin::default().persistence(PersistenceConfig::new("test").with_path(save_path)),
    );

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

  fn world_mut(&mut self) -> Mut<'_, PixelWorld> {
    let mut q = self.app.world_mut().query::<&mut PixelWorld>();
    q.single_mut(self.app.world_mut()).unwrap()
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

  /// Spawns a pixel body at the given position.
  fn spawn_pixel_body(
    &mut self,
    position: Vec2,
    size: u32,
    material: bevy_pixel_world::MaterialId,
  ) -> Entity {
    let mut body = PixelBody::new(size, size);

    // Fill with solid pixels
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

    let entity = self
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
      .id();

    entity
  }

  /// Gets the solid count for a specific body entity.
  fn body_solid_count(&self, entity: Entity) -> Option<usize> {
    self
      .app
      .world()
      .get::<PixelBody>(entity)
      .map(|b| b.solid_count())
  }

  /// Counts total number of pixel bodies.
  fn count_pixel_bodies(&mut self) -> usize {
    let mut q = self.app.world_mut().query::<&PixelBody>();
    q.iter(self.app.world()).count()
  }

  /// Gets all pixel body entities and their solid counts.
  fn get_all_bodies(&mut self) -> Vec<(Entity, usize)> {
    let mut q = self.app.world_mut().query::<(Entity, &PixelBody)>();
    q.iter(self.app.world())
      .map(|(e, b)| (e, b.solid_count()))
      .collect()
  }

  /// Erases a circular area in the world (simulates brush erasure).
  fn erase_circle(&mut self, center: WorldPos, radius: i64) {
    let void = Pixel::VOID;
    let rect = WorldRect::centered(center.x, center.y, radius as u32);

    let mut world = self.world_mut();
    world.blit(
      rect,
      |frag| {
        let dx = frag.x - center.x;
        let dy = frag.y - center.y;
        if dx * dx + dy * dy <= radius * radius {
          Some(void)
        } else {
          None
        }
      },
      Default::default(),
    );
  }
}

/// Test that pixel bodies do not spontaneously lose pixels after chunk seeding.
///
/// This test verifies the fix for the disintegration bug where async chunk
/// seeding would overwrite PIXEL_BODY flagged pixels, causing bodies to lose
/// their pixels.
#[test]
fn bodies_do_not_spontaneously_disintegrate() {
  let temp_dir = TempDir::new().unwrap();
  let save_path = temp_dir.path().join("stability.save");

  let mut harness = TestHarness::new(&save_path);

  // Center camera at origin
  harness.move_camera(Vec3::ZERO);

  // Run a few frames to start seeding
  harness.run(5);

  // Spawn several pixel bodies at positions that will be in seeding chunks
  let body_size = 8u32;
  let expected_solid = (body_size * body_size) as usize;

  let bodies: Vec<Entity> = (0..5)
    .map(|i| {
      let x = (i as f32 - 2.0) * 50.0;
      harness.spawn_pixel_body(Vec2::new(x, 0.0), body_size, material_ids::STONE)
    })
    .collect();

  // Verify initial solid counts
  for &body in &bodies {
    let solid = harness.body_solid_count(body).expect("Body should exist");
    assert_eq!(
      solid, expected_solid,
      "Initial solid count should be {}",
      expected_solid
    );
  }

  // Run simulation for many frames - this allows async seeding to complete
  // The bug occurs when seeding finishes and overwrites body pixels
  harness.run(100);

  // Verify bodies still have same solid count (no disintegration)
  for &body in &bodies {
    let solid = harness.body_solid_count(body);
    // Body may have despawned if fully destroyed - that's the bug we're testing for
    if let Some(count) = solid {
      assert_eq!(
        count, expected_solid,
        "Body solid count should remain {} after simulation, but was {}",
        expected_solid, count
      );
    } else {
      panic!("Body was despawned - this indicates spontaneous disintegration bug");
    }
  }

  // Verify body count is still 5
  assert_eq!(
    harness.count_pixel_bodies(),
    5,
    "All 5 bodies should still exist"
  );
}

/// Test that erased bodies are fully removed with no ghost pixels remaining.
///
/// This verifies that once a body's pixels are erased, the body entity is
/// properly despawned and doesn't leave behind unerasable pixels.
#[test]
fn erased_bodies_fully_removed() {
  let temp_dir = TempDir::new().unwrap();
  let save_path = temp_dir.path().join("erasure.save");

  let mut harness = TestHarness::new(&save_path);
  harness.move_camera(Vec3::ZERO);
  harness.run_until_seeded();

  // Spawn bodies
  let body_size = 8u32;
  let positions: Vec<Vec2> = (0..3)
    .map(|i| Vec2::new((i as f32 - 1.0) * 100.0, 0.0))
    .collect();

  for pos in &positions {
    harness.spawn_pixel_body(*pos, body_size, material_ids::STONE);
  }

  // Let bodies blit to world
  harness.run(20);

  assert_eq!(
    harness.count_pixel_bodies(),
    3,
    "Should have 3 bodies before erasure"
  );

  // Erase all body positions with generous radius
  for pos in &positions {
    let center = WorldPos::new(pos.x as i64, pos.y as i64);
    harness.erase_circle(center, 20);
  }

  // Run many frames to let the split system detect and despawn empty bodies
  harness.run(50);

  // All bodies should be despawned
  let remaining = harness.count_pixel_bodies();
  assert_eq!(
    remaining, 0,
    "All erased bodies should be despawned, but {} remain",
    remaining
  );
}

/// Stress test: spawn bodies while seeding is in progress.
///
/// This specifically targets the race condition where body pixels get blitted
/// to a chunk before seeding completes, then seeding replaces all pixels.
#[test]
fn bodies_survive_concurrent_seeding() {
  let temp_dir = TempDir::new().unwrap();
  let save_path = temp_dir.path().join("concurrent.save");

  let mut harness = TestHarness::new(&save_path);
  harness.move_camera(Vec3::ZERO);

  // Don't wait for seeding - spawn bodies immediately while chunks are being
  // seeded
  let body_size = 10u32;
  let expected_solid = (body_size * body_size) as usize;

  // Spawn a body right at the start
  let body = harness.spawn_pixel_body(Vec2::new(0.0, 0.0), body_size, material_ids::STONE);

  // Verify it starts with correct pixel count
  assert_eq!(
    harness.body_solid_count(body).unwrap(),
    expected_solid,
    "Body should start with {} pixels",
    expected_solid
  );

  // Run many frames to allow seeding to complete
  for frame in 0..200 {
    harness.run(1);

    // Periodically check the body is still intact
    if frame % 20 == 19 {
      if let Some(count) = harness.body_solid_count(body) {
        assert_eq!(
          count, expected_solid,
          "Body should maintain {} pixels at frame {}, but has {}",
          expected_solid, frame, count
        );
      } else {
        panic!("Body was despawned at frame {} - disintegration bug", frame);
      }
    }
  }

  // Final verification
  let final_count = harness
    .body_solid_count(body)
    .expect("Body should still exist after 200 frames");
  assert_eq!(
    final_count, expected_solid,
    "Body should end with {} pixels",
    expected_solid
  );
}
