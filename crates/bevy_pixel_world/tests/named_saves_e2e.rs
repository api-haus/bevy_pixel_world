//! E2E tests for persistence functionality.
//!
//! Tests:
//! - Basic save operations (save to current path, save creates file)
//! - Copy-on-write semantics (save_to different path creates copy)
//! - Save/load cycle verification

use std::path::PathBuf;

use bevy::app::{TaskPoolOptions, TaskPoolPlugin};
use bevy::ecs::world::Mut;
use bevy::prelude::*;
use bevy_pixel_world::{
  CHUNK_SIZE, ColorIndex, MaterialSeeder, PersistenceConfig, PersistenceControl, PersistenceHandle,
  Pixel, PixelWorld, PixelWorldPlugin, SpawnPixelWorld, StreamingCamera, WorldPos,
  debug_shim::DebugGizmos, material_ids,
};
use tempfile::TempDir;

/// Camera speed in pixels per simulated second
const CAMERA_SPEED: f32 = 500.0;
/// Simulated frame delta (60 FPS)
const DELTA_TIME: f32 = 1.0 / 60.0;

struct PersistenceHarness {
  app: App,
  camera: Entity,
  temp_dir: TempDir,
  save_path: PathBuf,
}

impl PersistenceHarness {
  /// Creates a harness with persistence at a specific path.
  fn new(save_name: &str) -> Self {
    let temp_dir = TempDir::new().unwrap();
    Self::with_temp_dir(temp_dir, save_name)
  }

  /// Creates a harness using an existing temp directory.
  fn with_temp_dir(temp_dir: TempDir, save_name: &str) -> Self {
    let save_path = temp_dir.path().join(format!("{}.save", save_name));
    let mut app = App::new();
    app.add_plugins(MinimalPlugins.set(TaskPoolPlugin {
      task_pool_options: TaskPoolOptions::with_num_threads(4),
    }));

    // Configure persistence with absolute path
    let config = PersistenceConfig::at(&save_path);

    app.add_plugins(PixelWorldPlugin::new(config));

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

    Self {
      app,
      camera,
      temp_dir,
      save_path,
    }
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

  fn persistence_control(&mut self) -> Mut<'_, PersistenceControl> {
    self.app.world_mut().resource_mut::<PersistenceControl>()
  }

  /// Triggers a save and runs updates until it completes.
  fn save_and_wait(&mut self) -> PersistenceHandle {
    let handle = self.persistence_control().save();
    // Run updates until save completes
    for _ in 0..100 {
      self.app.update();
      if handle.is_complete() {
        return handle;
      }
    }
    panic!("Save did not complete within 100 updates");
  }

  /// Triggers a save_to and runs updates until it completes.
  fn save_to_and_wait(&mut self, path: PathBuf) -> PersistenceHandle {
    let handle = self.persistence_control().save_to(path);
    // Run updates until save completes
    for _ in 0..100 {
      self.app.update();
      if handle.is_complete() {
        return handle;
      }
    }
    panic!("Save did not complete within 100 updates");
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

  /// Scroll naturally like holding WASD keys
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

  /// Returns whether a save file exists at the given path.
  fn path_exists(&self, path: &PathBuf) -> bool {
    path.exists()
  }

  /// Returns the size of a save file.
  fn save_file_size(&self, path: &PathBuf) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
  }

  /// Paint a circle of stone pixels at the given position with the given color.
  fn paint_circle(&mut self, center: WorldPos, color: ColorIndex, radius: i64) {
    let mut world = self.world_mut();
    for dy in -radius..=radius {
      for dx in -radius..=radius {
        if dx * dx + dy * dy <= radius * radius {
          world.set_pixel(
            WorldPos::new(center.x + dx, center.y + dy),
            Pixel::new(material_ids::STONE, color),
            DebugGizmos::none(),
          );
        }
      }
    }
  }

  /// Verify a pixel exists at the given position with the expected color.
  fn verify_pixel(&mut self, pos: WorldPos, expected_color: ColorIndex) -> bool {
    self
      .world()
      .get_pixel(pos)
      .is_some_and(|p| p.material == material_ids::STONE && p.color == expected_color)
  }

  /// Flush chunks to disk by scrolling away (unloads chunks, triggering
  /// persistence).
  fn flush_to_disk(&mut self) {
    let far_right = Vec3::new(5.0 * CHUNK_SIZE as f32, 0.0, 0.0);
    self.scroll_to(far_right);
    self.run(30);
  }
}

// =============================================================================
// Basic Save Operations
// =============================================================================

#[test]
fn save_persists_chunks() {
  let mut harness = PersistenceHarness::new("world");
  harness.run_until_seeded();

  // Paint markers
  let markers = [
    (WorldPos::new(64, 64), ColorIndex(1)),
    (WorldPos::new(80, 64), ColorIndex(2)),
  ];

  for (pos, color) in &markers {
    harness.paint_circle(*pos, *color, 5);
  }

  // Save to current path
  harness.save_and_wait();

  // Scroll away to unload chunks
  let far_right = Vec3::new(5.0 * CHUNK_SIZE as f32, 0.0, 0.0);
  harness.scroll_to(far_right);
  harness.run(30);

  // Verify markers unloaded
  for (pos, _) in &markers {
    assert!(harness.world().get_pixel(*pos).is_none());
  }

  // Scroll back
  harness.scroll_to(Vec3::ZERO);
  harness.run(30);

  // Verify markers restored
  for (pos, color) in &markers {
    assert!(
      harness.verify_pixel(*pos, *color),
      "Marker at {:?} should be restored",
      pos
    );
  }
}

#[test]
fn save_creates_file() {
  let mut harness = PersistenceHarness::new("world");
  harness.run_until_seeded();

  // Paint something
  harness.paint_circle(WorldPos::new(64, 64), ColorIndex(1), 5);

  // Save
  harness.save_and_wait();

  // Verify file exists
  assert!(
    harness.path_exists(&harness.save_path),
    "{:?} should exist",
    harness.save_path
  );
}

#[test]
fn is_active_returns_true_when_save_loaded() {
  let mut harness = PersistenceHarness::new("myworld");

  let persistence = harness.persistence_control();
  assert!(persistence.is_active(), "persistence should be active");
}

// =============================================================================
// Copy-on-Write
// =============================================================================

#[test]
#[ignore = "copy-on-write requires IoDispatcher CopyTo command (not yet implemented)"]
fn save_to_creates_copy() {
  let mut harness = PersistenceHarness::new("primary");
  harness.run_until_seeded();

  // Paint markers
  harness.paint_circle(WorldPos::new(64, 64), ColorIndex(1), 5);

  // Save to primary first
  harness.save_and_wait();
  harness.flush_to_disk();
  assert!(
    harness.path_exists(&harness.save_path),
    "primary.save should exist after flush"
  );

  // Return to origin to reload chunks
  harness.scroll_to(Vec3::ZERO);
  harness.run(30);

  // Paint more
  harness.paint_circle(WorldPos::new(80, 80), ColorIndex(2), 5);

  // Save to backup (copy-on-write)
  let backup_path = harness.temp_dir.path().join("backup.save");
  harness.save_to_and_wait(backup_path.clone());
  harness.flush_to_disk();

  // Verify backup file exists
  assert!(
    harness.path_exists(&backup_path),
    "backup.save should exist"
  );
}

#[test]
#[ignore = "copy-on-write requires IoDispatcher CopyTo command (not yet implemented)"]
fn copy_on_write_source_unchanged() {
  let mut harness = PersistenceHarness::new("source");
  harness.run_until_seeded();

  // Paint marker A
  harness.paint_circle(WorldPos::new(64, 64), ColorIndex(1), 5);

  // Save to source (flush)
  harness.save_and_wait();
  let source_size_after_a = harness.save_file_size(&harness.save_path);

  // Paint marker B
  harness.paint_circle(WorldPos::new(128, 128), ColorIndex(2), 5);

  // Save to target (copy-on-write)
  let target_path = harness.temp_dir.path().join("target.save");
  harness.save_to_and_wait(target_path.clone());

  // Source file should remain unchanged
  let source_size_after_cow = harness.save_file_size(&harness.save_path);
  assert_eq!(
    source_size_after_a, source_size_after_cow,
    "Source file should not change during copy-on-write"
  );

  // Target should exist
  assert!(
    harness.path_exists(&target_path),
    "target.save should exist"
  );
}

#[test]
#[ignore = "copy-on-write requires IoDispatcher CopyTo command (not yet implemented)"]
fn multiple_snapshots_independent() {
  let mut temp_dir = TempDir::new().unwrap();

  // Create v1 snapshot
  {
    let mut harness = PersistenceHarness::with_temp_dir(temp_dir, "v1");
    harness.run_until_seeded();

    // Paint circle 1 at (64, 64) with color 1
    harness.paint_circle(WorldPos::new(64, 64), ColorIndex(1), 5);
    harness.save_and_wait();

    // Paint circle 2 at (128, 128) with color 2
    harness.paint_circle(WorldPos::new(128, 128), ColorIndex(2), 5);

    // Save to v2 (copy-on-write)
    let v2_path = harness.temp_dir.path().join("v2.save");
    harness.save_to_and_wait(v2_path);

    // Verify both circles visible in current session
    assert!(harness.verify_pixel(WorldPos::new(64, 64), ColorIndex(1)));
    assert!(harness.verify_pixel(WorldPos::new(128, 128), ColorIndex(2)));

    // Take ownership back
    temp_dir = harness.temp_dir;
  }

  // Reload from v1 - should only have first circle
  {
    let mut harness = PersistenceHarness::with_temp_dir(temp_dir, "v1");
    harness.run_until_seeded();
    harness.run(30);

    assert!(
      harness.verify_pixel(WorldPos::new(64, 64), ColorIndex(1)),
      "v1 should have first circle"
    );
    // Second circle should not exist in v1
    assert!(
      !harness.verify_pixel(WorldPos::new(128, 128), ColorIndex(2)),
      "v1 should not have second circle"
    );

    temp_dir = harness.temp_dir;
  }

  // Reload from v2 - should have both circles
  {
    let mut harness = PersistenceHarness::with_temp_dir(temp_dir, "v2");
    harness.run_until_seeded();
    harness.run(30);

    assert!(
      harness.verify_pixel(WorldPos::new(64, 64), ColorIndex(1)),
      "v2 should have first circle"
    );
    assert!(
      harness.verify_pixel(WorldPos::new(128, 128), ColorIndex(2)),
      "v2 should have second circle"
    );
  }
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn save_seeded_world_succeeds() {
  let mut harness = PersistenceHarness::new("world");
  harness.run_until_seeded();

  // Don't paint anything - test saving a world with no user modifications
  // (still has seeded terrain)

  let handle = harness.save_and_wait();
  harness.flush_to_disk();
  assert!(handle.is_complete(), "Save should complete");

  // File should exist
  assert!(
    harness.path_exists(&harness.save_path),
    "world.save should exist"
  );
}

// Note: Persistence is always enabled in the new architecture.
// There is no "disabled" mode - a path must always be provided.
