//! Full Bevy E2E persistence test.
//!
//! Tests the complete persistence flow:
//! 1. Paint a blob of pixels
//! 2. Scroll camera away (chunk leaves streaming window -> saved to disk)
//! 3. Scroll camera back (chunk re-enters -> loaded from persistence)
//! 4. Verify painted blob is restored

use bevy::app::{TaskPoolOptions, TaskPoolPlugin};
use bevy::ecs::world::Mut;
use bevy::prelude::*;
use bevy_pixel_world::{
  CHUNK_SIZE, ColorIndex, MaterialSeeder, PersistenceConfig, Pixel, PixelWorld, PixelWorldPlugin,
  SpawnPixelWorld, StreamingCamera, WorldPos, debug_shim::DebugGizmos, material_ids,
};

/// Camera speed in pixels per simulated second (matches painting demo)
const CAMERA_SPEED: f32 = 500.0;
/// Simulated frame delta (60 FPS)
const DELTA_TIME: f32 = 1.0 / 60.0;
use std::path::Path;

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

    app.update(); // Apply spawn command

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

  fn world(&mut self) -> &PixelWorld {
    let mut q = self.app.world_mut().query::<&PixelWorld>();
    q.single(self.app.world()).unwrap()
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
    // MinimalPlugins doesn't run transform propagation
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

  /// Scroll naturally like holding WASD keys in the painting demo
  fn scroll_to(&mut self, target: Vec3) {
    let speed = CAMERA_SPEED * DELTA_TIME; // ~8.3 pixels per update

    loop {
      let current = self.camera_position();
      let delta = target - current;

      // Close enough - snap to target
      if delta.length() < speed {
        self.move_camera(target);
        self.app.update();
        break;
      }

      // Move incrementally toward target
      let direction = delta.normalize();
      let new_pos = current + direction * speed;
      self.move_camera(new_pos);
      self.app.update();
    }
  }
}

/// This test uses MinimalPlugins which doesn't provide GizmoConfigStore.
/// Run with: cargo test --features headless --no-default-features
#[test]
fn painted_chunks_persist_across_scroll() {
  let temp_dir = TempDir::new().unwrap();
  let save_path = temp_dir.path().join("test.save");

  let mut harness = TestHarness::new(&save_path);
  harness.run_until_seeded();

  // Paint markers in 3 different chunks using STONE (solid, doesn't move during
  // sim) Distinguish chunks by ColorIndex.
  // Markers must be >10 pixels apart so radius-5 circles don't overlap.
  let markers = [
    (WorldPos::new(64, 64), ColorIndex(1)),         // Chunk (0, 0)
    (WorldPos::new(80, 64), ColorIndex(2)),         // Chunk (0, 0) - 16 pixels right
    (WorldPos::new(64, 80), ColorIndex(3)),         // Chunk (0, 0) - 16 pixels up
    (WorldPos::new(80, 80), ColorIndex(4)),         // Chunk (0, 0) - diagonal
    (WorldPos::new(96, 64), ColorIndex(5)),         // Chunk (0, 0) - 32 pixels right
    (WorldPos::new(512 + 64, 64), ColorIndex(150)), // Chunk (1, 0)
    (WorldPos::new(-512 + 64, 64), ColorIndex(200)), // Chunk (-1, 0)
  ];

  {
    let mut world = harness.world_mut();
    for (pos, color) in &markers {
      for dy in -5i64..=5 {
        for dx in -5i64..=5 {
          if dx * dx + dy * dy <= 25 {
            world.set_pixel(
              WorldPos::new(pos.x + dx, pos.y + dy),
              Pixel::new(material_ids::STONE, *color),
              DebugGizmos::none(),
            );
          }
        }
      }
    }
  }

  // Verify all markers painted
  for (pos, color) in &markers {
    let pixel = harness.world().get_pixel(*pos).unwrap();
    assert_eq!(pixel.material, material_ids::STONE);
    assert_eq!(pixel.color, *color);
  }

  harness.run(10);

  // Scroll right naturally (simulates holding D key)
  // Window is 4x3 chunks, so we need to move ~5 chunk widths to ensure all 3
  // chunks unload
  let far_right = Vec3::new(5.0 * CHUNK_SIZE as f32, 0.0, 0.0);
  harness.scroll_to(far_right);

  // Extra updates to ensure persistence flush completes
  harness.run(30);

  // Verify all markers are now unloaded
  for (pos, _) in &markers {
    assert!(harness.world().get_pixel(*pos).is_none());
  }
  assert!(save_path.exists());

  // Scroll back naturally (simulates holding A key)
  harness.scroll_to(Vec3::ZERO);

  // Extra updates to ensure loading completes
  harness.run(30);

  // Verify all markers restored with correct color
  for (pos, color) in &markers {
    let pixel = harness.world().get_pixel(*pos);
    assert!(
      pixel.is_some(),
      "Pixel at {:?} should exist after restore",
      pos
    );
    let pixel = pixel.unwrap();
    assert_eq!(
      pixel.material,
      material_ids::STONE,
      "Marker at {:?} should be STONE",
      pos
    );
    assert_eq!(
      pixel.color, *color,
      "Marker at {:?} should have color {:?}",
      pos, color
    );
  }
}
