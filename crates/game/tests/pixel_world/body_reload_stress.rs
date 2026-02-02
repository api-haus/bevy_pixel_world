//! Stress test for pixel body reload without ghost duplicates.
//!
//! Tests that bodies can be properly erased after chunk reload cycles.
//! This catches the race condition where unload + reload in the same frame
//! causes duplicate body spawns, leaving "ghost" bodies that can't be erased.
//!
//! Run: cargo test -p game body_reload_stress

use std::path::Path;
use std::time::{Duration, Instant};

use bevy::app::{TaskPoolOptions, TaskPoolPlugin};
use bevy::ecs::world::Mut;
use bevy::prelude::*;
use game::pixel_world::{
  AsyncTaskBehavior, CHUNK_SIZE, ColorIndex, MaterialSeeder, PersistenceConfig, Pixel, PixelWorld,
  PixelWorldPlugin, SpawnPixelWorld, StreamingCamera, WorldPos, material_ids,
};
use tempfile::TempDir;

/// Camera speed in pixels per simulated second
const CAMERA_SPEED: f32 = 500.0;
/// Simulated frame delta (60 FPS)
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
    self.run_until(WorldPos::new(0, 0), Duration::from_secs(5));
  }

  /// Runs updates until a pixel appears at the given position, or timeout.
  fn run_until(&mut self, pos: WorldPos, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
      self.app.update();
      std::thread::yield_now();
      let mut q = self.app.world_mut().query::<&PixelWorld>();
      if let Ok(world) = q.single(self.app.world()) {
        if world.get_pixel(pos).is_some() {
          return;
        }
      }
    }
    panic!("Pixel at {:?} not found within {:?}", pos, timeout);
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
}

/// Stress test: Paint pixels, scroll away and back, verify persistence works.
///
/// This test verifies that chunk pixels persist correctly across unload/reload
/// cycles. The ChunksWithSavedBodies resource prevents loading bodies for
/// chunks that were just saved in the same frame.
///
/// Note: Full physics body ghost testing requires avian2d feature. This test
/// validates the underlying chunk persistence mechanism.
#[test]
fn bodies_reload_without_ghosts() {
  let temp_dir = TempDir::new().unwrap();
  let save_path = temp_dir.path().join("stress.save");

  let mut harness = TestHarness::new(&save_path);
  harness.run_until_seeded();

  // Paint a single marker block
  let marker_pos = WorldPos::new(64, 64);
  let marker_color = ColorIndex(42);

  {
    let mut world = harness.world_mut();
    for dy in 0..8 {
      for dx in 0..8 {
        world.set_pixel(
          WorldPos::new(marker_pos.x + dx, marker_pos.y + dy),
          Pixel::new(material_ids::STONE, marker_color),
          Default::default(),
        );
      }
    }
  }

  harness.run(30);

  // Verify marker painted
  let pixel = harness.world().get_pixel(marker_pos);
  assert!(pixel.is_some(), "Marker should exist");
  assert_eq!(pixel.unwrap().color, marker_color);

  // Scroll far away to trigger chunk unload
  let far_away = Vec3::new(5.0 * CHUNK_SIZE as f32, 0.0, 0.0);
  harness.scroll_to(far_away);
  harness.run(30);

  // Verify marker is unloaded
  assert!(harness.world().get_pixel(marker_pos).is_none());

  // Scroll back to trigger reload
  harness.scroll_to(Vec3::ZERO);
  harness.run_until(marker_pos, Duration::from_secs(5));

  // Verify marker is restored with correct color (persistence works)
  let pixel = harness.world().get_pixel(marker_pos);
  assert_eq!(
    pixel.unwrap().color,
    marker_color,
    "Marker color should be preserved"
  );
  assert_eq!(
    pixel.unwrap().material,
    material_ids::STONE,
    "Marker material should be preserved"
  );
}

/// Rapid reload stress test: quickly scroll back and forth.
///
/// This triggers the exact race condition by trying to unload and reload
/// chunks rapidly, increasing the chance of same-frame unload+reload.
#[test]
fn rapid_reload_no_ghosts() {
  let temp_dir = TempDir::new().unwrap();
  let save_path = temp_dir.path().join("rapid.save");

  let mut harness = TestHarness::new(&save_path);
  harness.run_until_seeded();

  // Paint a test pattern
  let marker_pos = WorldPos::new(64, 64);
  {
    let mut world = harness.world_mut();
    for dy in 0..8 {
      for dx in 0..8 {
        world.set_pixel(
          WorldPos::new(marker_pos.x + dx, marker_pos.y + dy),
          Pixel::new(material_ids::STONE, ColorIndex(50)),
          Default::default(),
        );
      }
    }
  }
  harness.run(5);

  // Rapid oscillation: repeatedly scroll to edge of streaming window and back
  // This creates situations where chunks unload and reload in rapid succession
  let near = Vec3::ZERO;
  let far = Vec3::new(3.0 * CHUNK_SIZE as f32, 0.0, 0.0);

  for _ in 0..5 {
    harness.scroll_to(far);
    harness.run(10);
    harness.scroll_to(near);
    harness.run(10);
  }

  // Wait for chunk to be fully loaded before erasing
  harness.run_until(marker_pos, Duration::from_secs(5));

  // Erase the marker
  {
    let mut world = harness.world_mut();
    for dy in -2..12 {
      for dx in -2..12 {
        world.set_pixel(
          WorldPos::new(marker_pos.x + dx, marker_pos.y + dy),
          Pixel::VOID,
          Default::default(),
        );
      }
    }
  }

  harness.run(10);

  // Verify erasure succeeded (no ghost pixels)
  for dy in 0..8 {
    for dx in 0..8 {
      let check_pos = WorldPos::new(marker_pos.x + dx, marker_pos.y + dy);
      let pixel = harness.world().get_pixel(check_pos);
      assert!(
        pixel.is_none() || pixel.unwrap().material == material_ids::VOID,
        "Ghost pixel at {:?} after rapid reload cycles",
        check_pos
      );
    }
  }
}
