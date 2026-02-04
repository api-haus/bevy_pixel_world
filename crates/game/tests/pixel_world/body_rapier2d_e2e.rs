//! E2E test for pixel body persistence with rapier2d physics.
//!
//! Tests that pixel bodies with rapier2d colliders survive chunk unload/reload
//! without causing parry2d BVH panics or losing body data.
//!
//! Run: cargo test -p game --features rapier2d body_rapier2d_e2e

use std::path::Path;

use bevy::app::{TaskPoolOptions, TaskPoolPlugin};
use bevy::prelude::*;
use game::pixel_world::{
  CHUNK_SIZE, ColorIndex, DisplacementState, LastBlitTransform, MaterialSeeder, Persistable,
  PersistenceConfig, Pixel, PixelBodiesPlugin, PixelBody, PixelBodyIdGenerator, PixelWorld,
  PixelWorldPlugin, SpawnPixelWorld, StreamingCamera, WorldPos, material_ids,
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

    // Add rapier2d physics
    app.add_plugins(
      bevy_rapier2d::prelude::RapierPhysicsPlugin::<bevy_rapier2d::prelude::NoUserData>::default()
        .with_length_unit(50.0),
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
    material: game::pixel_world::MaterialId,
  ) -> Entity {
    let mut body = PixelBody::new(size, size);

    for y in 0..size {
      for x in 0..size {
        body.set_pixel(x, y, Pixel::new(material, ColorIndex(100)));
      }
    }

    let collider =
      game::pixel_world::generate_collider(&body).expect("body should produce a valid collider");

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
        collider,
        bevy_rapier2d::prelude::RigidBody::KinematicPositionBased,
        bevy_rapier2d::prelude::Velocity::default(),
        game::pixel_world::CollisionQueryPoint,
        game::pixel_world::StreamCulled,
      ))
      .id()
  }

  fn count_pixel_bodies(&mut self) -> usize {
    let mut q = self.app.world_mut().query::<&PixelBody>();
    q.iter(self.app.world()).count()
  }
}

/// Test that pixel bodies with rapier2d physics survive chunk unload/reload
/// without causing parry2d BVH panics.
///
/// Bodies fall due to gravity, so this tests the realistic scenario where
/// physics-active bodies need to be saved and restored with colliders.
#[test]
fn rapier2d_bodies_survive_chunk_reload() {
  let temp_dir = TempDir::new().unwrap();
  let save_path = temp_dir.path().join("rapier_persist.save");

  let mut harness = TestHarness::new(&save_path);
  harness.move_camera(Vec3::ZERO);
  harness.run_until_seeded();

  // Spawn several pixel bodies at known positions
  let body_size = 8u32;
  let expected_solid = (body_size * body_size) as usize;
  let positions = [
    Vec2::new(0.0, 50.0),
    Vec2::new(50.0, 50.0),
    Vec2::new(-50.0, 50.0),
    Vec2::new(0.0, 100.0),
    Vec2::new(100.0, 50.0),
  ];

  for &pos in &positions {
    harness.spawn_pixel_body(pos, body_size, material_ids::STONE);
  }

  // Let bodies blit and physics settle
  harness.run(30);

  // Verify all bodies exist before scrolling
  assert_eq!(harness.count_pixel_bodies(), positions.len());

  // Teleport far away to trigger chunk unload + body save
  let far_away = Vec3::new(5.0 * CHUNK_SIZE as f32, 0.0, 0.0);
  harness.move_camera(far_away);
  harness.run(60);

  assert_eq!(
    harness.count_pixel_bodies(),
    0,
    "All bodies should be despawned after moving away"
  );

  // Teleport back to trigger chunk reload + body load (with rapier2d physics)
  harness.move_camera(Vec3::ZERO);
  harness.run(120);

  // All bodies should survive the reload cycle
  let body_count = harness.count_pixel_bodies();
  assert_eq!(
    body_count,
    positions.len(),
    "Expected {} pixel bodies after reload, got {}",
    positions.len(),
    body_count
  );

  // Verify reloaded bodies have correct pixel data
  let mut q = harness
    .app
    .world_mut()
    .query::<(&PixelBody, &LastBlitTransform)>();
  let body_info: Vec<_> = q
    .iter(harness.app.world())
    .map(|(body, lbt)| (body.solid_count(), lbt.written_positions.len()))
    .collect();

  for (solid, written) in &body_info {
    assert_eq!(
      *solid, expected_solid,
      "Reloaded body should have {expected_solid} solid pixels, got {solid}"
    );
    assert!(
      *written > 0,
      "Reloaded body should have written_positions, got 0"
    );
  }
}
