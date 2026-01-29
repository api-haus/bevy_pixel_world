//! E2E tests for named saves functionality.
//!
//! Tests:
//! - Basic save operations (save to current name, save creates file)
//! - Copy-on-write semantics (save to different name creates copy)
//! - Save management API (list, delete, path queries)
//! - Edge cases (empty world, current save tracking)

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

struct NamedSavesHarness {
  app: App,
  camera: Entity,
  temp_dir: TempDir,
}

impl NamedSavesHarness {
  /// Creates a harness that will load from the specified save name.
  fn new(load_save: &str) -> Self {
    let temp_dir = TempDir::new().unwrap();
    Self::with_temp_dir(temp_dir, load_save)
  }

  /// Creates a harness using an existing temp directory, loading from the
  /// specified save.
  fn with_temp_dir(temp_dir: TempDir, load_save: &str) -> Self {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins.set(TaskPoolPlugin {
      task_pool_options: TaskPoolOptions::with_num_threads(4),
    }));

    // Configure persistence with temp directory as base and specified save to load
    let config = PersistenceConfig::new("test")
      .with_path(temp_dir.path().join(format!("{}.save", load_save)))
      .load(load_save);

    app.add_plugins(PixelWorldPlugin::default().persistence(config));

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
  fn save_and_wait(&mut self, name: &str) -> PersistenceHandle {
    let handle = self.persistence_control().save(name);
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

  /// Returns whether a save file exists.
  fn save_exists(&self, name: &str) -> bool {
    self.save_path(name).exists()
  }

  /// Returns the size of a save file.
  fn save_file_size(&self, name: &str) -> u64 {
    std::fs::metadata(self.save_path(name))
      .map(|m| m.len())
      .unwrap_or(0)
  }

  /// Returns the path for a named save file.
  fn save_path(&self, name: &str) -> PathBuf {
    self.temp_dir.path().join(format!("{}.save", name))
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
fn save_to_loaded_name_persists_chunks() {
  let mut harness = NamedSavesHarness::new("world");
  harness.run_until_seeded();

  // Paint markers
  let markers = [
    (WorldPos::new(64, 64), ColorIndex(1)),
    (WorldPos::new(80, 64), ColorIndex(2)),
  ];

  for (pos, color) in &markers {
    harness.paint_circle(*pos, *color, 5);
  }

  // Save to the loaded name
  harness.save_and_wait("world");

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
fn save_creates_named_file() {
  let mut harness = NamedSavesHarness::new("world");
  harness.run_until_seeded();

  // Paint something
  harness.paint_circle(WorldPos::new(64, 64), ColorIndex(1), 5);

  // Save to custom name
  harness.save_and_wait("custom");

  // Verify file exists
  assert!(harness.save_exists("custom"), "custom.save should exist");
}

// =============================================================================
// Copy-on-Write
// =============================================================================

#[test]
fn save_to_different_name_creates_copy() {
  let mut harness = NamedSavesHarness::new("primary");
  harness.run_until_seeded();

  // Paint markers
  harness.paint_circle(WorldPos::new(64, 64), ColorIndex(1), 5);

  // Save to primary first (to create the source file)
  harness.save_and_wait("primary");

  // Flush to write primary.save
  harness.flush_to_disk();
  assert!(
    harness.save_exists("primary"),
    "primary.save should exist after flush"
  );

  // Return to origin to reload chunks
  harness.scroll_to(Vec3::ZERO);
  harness.run(30);

  // Paint a different marker to mark chunks dirty for backup save
  harness.paint_circle(WorldPos::new(80, 80), ColorIndex(2), 5);

  // Now save to backup (copy-on-write from primary)
  harness.save_and_wait("backup");

  // Flush to write backup.save
  harness.flush_to_disk();

  // Verify backup file exists
  assert!(harness.save_exists("backup"), "backup.save should exist");
}

#[test]
fn copy_on_write_source_unchanged() {
  let mut harness = NamedSavesHarness::new("source");
  harness.run_until_seeded();

  // Paint marker A
  harness.paint_circle(WorldPos::new(64, 64), ColorIndex(1), 5);

  // Save to source (flush)
  harness.save_and_wait("source");
  let source_size_after_a = harness.save_file_size("source");

  // Paint marker B
  harness.paint_circle(WorldPos::new(128, 128), ColorIndex(2), 5);

  // Save to target (copy-on-write)
  harness.save_and_wait("target");

  // Source file should remain unchanged (marker B not written to source)
  let source_size_after_cow = harness.save_file_size("source");
  assert_eq!(
    source_size_after_a, source_size_after_cow,
    "Source file should not change during copy-on-write"
  );

  // Target should exist with different content
  assert!(harness.save_exists("target"), "target.save should exist");
}

#[test]
fn multiple_snapshots_independent() {
  let mut temp_dir = TempDir::new().unwrap();

  // Create v1 snapshot
  {
    let mut harness = NamedSavesHarness::with_temp_dir(temp_dir, "v1");
    harness.run_until_seeded();

    // Paint circle 1 at (64, 64) with color 1
    harness.paint_circle(WorldPos::new(64, 64), ColorIndex(1), 5);
    harness.save_and_wait("v1");

    // Paint circle 2 at (128, 128) with color 2
    harness.paint_circle(WorldPos::new(128, 128), ColorIndex(2), 5);
    harness.save_and_wait("v2");

    // Verify both circles visible in current session
    assert!(harness.verify_pixel(WorldPos::new(64, 64), ColorIndex(1)));
    assert!(harness.verify_pixel(WorldPos::new(128, 128), ColorIndex(2)));

    // Explicitly take ownership back to drop harness and keep temp_dir
    temp_dir = harness.temp_dir;
  }

  // Reload from v1 - should only have first circle
  {
    let mut harness = NamedSavesHarness::with_temp_dir(temp_dir, "v1");
    harness.run_until_seeded();

    // Wait for chunks to load
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
    let mut harness = NamedSavesHarness::with_temp_dir(temp_dir, "v2");
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
// Save Management API
// =============================================================================

#[test]
fn list_saves_finds_all_saves() {
  let mut harness = NamedSavesHarness::new("world");
  harness.run_until_seeded();

  // Paint something so there's data to save
  harness.paint_circle(WorldPos::new(64, 64), ColorIndex(1), 5);

  // Save to alpha (copy-on-write from world)
  harness.save_and_wait("alpha");
  harness.flush_to_disk();
  harness.scroll_to(Vec3::ZERO);
  harness.run(30);

  // Paint more to mark chunks dirty for next save
  harness.paint_circle(WorldPos::new(80, 64), ColorIndex(2), 5);

  // Save to beta
  harness.save_and_wait("beta");
  harness.flush_to_disk();
  harness.scroll_to(Vec3::ZERO);
  harness.run(30);

  // Paint more
  harness.paint_circle(WorldPos::new(64, 80), ColorIndex(3), 5);

  // Save to gamma
  harness.save_and_wait("gamma");
  harness.flush_to_disk();

  // List saves
  let saves = harness.persistence_control().list_saves().unwrap();

  assert!(saves.contains(&"alpha".to_string()), "Should find alpha");
  assert!(saves.contains(&"beta".to_string()), "Should find beta");
  assert!(saves.contains(&"gamma".to_string()), "Should find gamma");
}

#[test]
fn list_saves_returns_existing_saves() {
  let temp_dir = TempDir::new().unwrap();

  // Pre-create some .save files to test list_saves
  std::fs::create_dir_all(temp_dir.path()).unwrap();
  std::fs::write(temp_dir.path().join("save1.save"), b"").unwrap();
  std::fs::write(temp_dir.path().join("save2.save"), b"").unwrap();

  // Create harness
  let mut harness = NamedSavesHarness::with_temp_dir(temp_dir, "world");

  let saves = harness.persistence_control().list_saves().unwrap();

  // Should find the saves we created (and possibly "world" if auto-created)
  assert!(saves.contains(&"save1".to_string()), "Should find save1");
  assert!(saves.contains(&"save2".to_string()), "Should find save2");
}

#[test]
fn delete_save_removes_file() {
  let mut harness = NamedSavesHarness::new("world");
  harness.run_until_seeded();

  // Paint something so there's data to save
  harness.paint_circle(WorldPos::new(64, 64), ColorIndex(1), 5);

  // Create a save to delete
  harness.save_and_wait("doomed");
  harness.flush_to_disk();
  assert!(harness.save_exists("doomed"), "doomed.save should exist");

  // Delete it
  let result = harness.persistence_control().delete_save("doomed");
  assert!(result.is_ok(), "delete_save should succeed");

  // Verify it's gone
  assert!(
    !harness.save_exists("doomed"),
    "doomed.save should be deleted"
  );
}

#[test]
fn delete_nonexistent_returns_error() {
  let mut harness = NamedSavesHarness::new("world");

  // Try to delete a save that doesn't exist
  let result = harness.persistence_control().delete_save("ghost");
  assert!(
    result.is_err(),
    "Deleting nonexistent save should return error"
  );
}

#[test]
fn save_file_name_correct() {
  let file_name = bevy_pixel_world::PersistenceControl::save_file_name("test");
  assert_eq!(
    file_name, "test.save",
    "save_file_name should return correct name"
  );
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn save_seeded_world_succeeds() {
  let mut harness = NamedSavesHarness::new("world");
  harness.run_until_seeded();

  // Don't paint anything - test saving a world with no user modifications
  // (still has seeded terrain)

  // Save to the same name should complete without error
  let handle = harness.save_and_wait("world");
  harness.flush_to_disk();
  assert!(handle.is_complete(), "Save should complete");

  // File should exist (contains seeded terrain data)
  assert!(harness.save_exists("world"), "world.save should exist");
}

#[test]
fn current_save_tracks_loaded_name() {
  let mut harness = NamedSavesHarness::new("myworld");

  let current = harness
    .persistence_control()
    .current_save()
    .expect("save should be loaded")
    .to_string();
  assert_eq!(current, "myworld", "current_save should match loaded name");
}
