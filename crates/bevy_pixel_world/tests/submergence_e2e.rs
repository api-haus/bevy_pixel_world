//! E2E tests for submergence detection.
//!
//! Tests that pixel bodies entering/exiting liquid emit appropriate messages
//! and have their `SubmersionState` updated correctly.
//!
//! Run with:
//!   cargo test -p bevy_pixel_world --test submergence_e2e \
//!     --features avian2d

use std::path::Path;

use bevy::app::{TaskPoolOptions, TaskPoolPlugin};
use bevy::asset::RenderAssetUsages;
use bevy::ecs::message::{MessageCursor, Messages};
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy_pixel_world::buoyancy::{
  Buoyancy2dPlugin, Submerged, Submergent, SubmersionState, Surfaced,
};
use bevy_pixel_world::debug_shim::DebugGizmos;
use bevy_pixel_world::pixel_awareness::PixelAwarenessPlugin;
use bevy_pixel_world::{
  ColorIndex, MaterialSeeder, PersistenceConfig, Pixel, PixelBodiesPlugin, PixelBody, PixelWorld,
  PixelWorldPlugin, SpawnPixelBodyFromImage, SpawnPixelWorld, StreamingCamera, WorldPos,
  material_ids,
};
use tempfile::TempDir;

/// Snapshot of submersion state (since SubmersionState doesn't impl Clone).
#[derive(Debug)]
struct SubmersionStateSnapshot {
  is_submerged: bool,
  submerged_fraction: f32,
  debug_liquid_samples: u32,
  debug_total_samples: u32,
}

/// Snapshot of Submerged message.
#[derive(Debug)]
struct SubmergedSnapshot {
  #[allow(dead_code)]
  entity: Entity,
  #[allow(dead_code)]
  submerged_fraction: f32,
}

/// Snapshot of Surfaced message.
#[derive(Debug)]
struct SurfacedSnapshot {
  #[allow(dead_code)]
  entity: Entity,
}

struct TestHarness {
  app: App,
  #[allow(dead_code)]
  camera: Entity,
  test_image: Handle<Image>,
  submerged_cursor: MessageCursor<Submerged>,
  surfaced_cursor: MessageCursor<Surfaced>,
}

impl TestHarness {
  fn new(save_path: &Path) -> Self {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins.set(TaskPoolPlugin {
      task_pool_options: TaskPoolOptions::with_num_threads(4),
    }));

    // Required plugins for transform and asset handling
    app.add_plugins(bevy::transform::TransformPlugin);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::image::ImagePlugin::default());
    app.add_plugins(bevy::scene::ScenePlugin);

    app.add_plugins(
      PixelWorldPlugin::default().persistence(PersistenceConfig::new("test").with_path(save_path)),
    );
    app.add_plugins(PixelBodiesPlugin);

    // Add physics plugins (required for submergence physics effects)
    #[cfg(feature = "avian2d")]
    {
      app.add_plugins(bevy::diagnostic::DiagnosticsPlugin);
      app.add_plugins(avian2d::prelude::PhysicsPlugins::default());
      app.insert_resource(avian2d::prelude::Gravity(Vec2::new(0.0, -500.0)));
      app.init_resource::<avian2d::collision::CollisionDiagnostics>();
      app.init_resource::<avian2d::dynamics::solver::SolverDiagnostics>();
      app.init_resource::<avian2d::spatial_query::SpatialQueryDiagnostics>();
    }

    #[cfg(all(feature = "rapier2d", not(feature = "avian2d")))]
    {
      app.add_plugins(
        bevy_rapier2d::prelude::RapierPhysicsPlugin::<bevy_rapier2d::prelude::NoUserData>::default(
        )
        .with_length_unit(50.0),
      );
    }

    // Add pixel awareness and buoyancy/submersion plugins
    app.add_plugins(PixelAwarenessPlugin::default());
    app.add_plugins(Buoyancy2dPlugin::default());

    // Create test image for spawning bodies
    let test_image = create_test_image(&mut app);

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

    Self {
      app,
      camera,
      test_image,
      submerged_cursor: MessageCursor::default(),
      surfaced_cursor: MessageCursor::default(),
    }
  }

  fn run(&mut self, updates: usize) {
    for _ in 0..updates {
      self.app.update();
    }
  }

  /// Runs updates until the world has seeded chunks around the camera.
  fn run_until_seeded(&mut self) {
    // Run enough frames for initial chunk seeding
    self.run(20);
  }

  /// Paints a rectangular pool of water in the world.
  fn paint_liquid_pool(&mut self, center: WorldPos, width: i64, height: i64) {
    let half_w = width / 2;
    let half_h = height / 2;

    let mut world = self.app.world_mut().query::<&mut PixelWorld>();
    let mut world = world.single_mut(self.app.world_mut()).unwrap();

    for dy in -half_h..=half_h {
      for dx in -half_w..=half_w {
        let pos = WorldPos::new(center.x + dx, center.y + dy);
        let pixel = Pixel::new(material_ids::WATER, ColorIndex(128));
        world.set_pixel(pos, pixel, DebugGizmos::default());
      }
    }
  }

  /// Spawns a pixel body at the given position.
  fn spawn_pixel_body(&mut self, position: Vec2) -> Entity {
    let image = self.test_image.clone();
    self
      .app
      .world_mut()
      .commands()
      .queue(SpawnPixelBodyFromImage::new(
        image,
        material_ids::WOOD,
        position,
      ));

    // Run frames to let the body finalize
    self.run(10);

    // Find the spawned body
    let mut q = self.app.world_mut().query::<(Entity, &PixelBody)>();
    q.iter(self.app.world())
      .next()
      .map(|(e, _)| e)
      .expect("Body should exist after spawning")
  }

  /// Teleports a body to a new position using physics backend's Position.
  fn teleport_body(&mut self, entity: Entity, position: Vec2) {
    // Use physics backend's position component for proper teleportation
    #[cfg(feature = "avian2d")]
    {
      use avian2d::prelude::*;
      // Set position directly via avian2d's Position component
      if let Some(mut pos) = self.app.world_mut().get_mut::<Position>(entity) {
        pos.0 = position;
      }
      // Also reset velocity
      if let Some(mut vel) = self.app.world_mut().get_mut::<LinearVelocity>(entity) {
        vel.0 = Vec2::ZERO;
      }
    }

    #[cfg(all(feature = "rapier2d", not(feature = "avian2d")))]
    {
      // For rapier, update Transform directly
      if let Some(mut transform) = self.app.world_mut().get_mut::<Transform>(entity) {
        transform.translation = position.extend(0.0);
      }
      if let Some(mut vel) = self
        .app
        .world_mut()
        .get_mut::<bevy_rapier2d::prelude::Velocity>(entity)
      {
        vel.linvel = Vec2::ZERO;
        vel.angvel = 0.0;
      }
    }

    // Run a frame to let physics sync Transform/GlobalTransform
    self.run(1);
  }

  /// Gets the submersion state for an entity (copies fields since
  /// SubmersionState doesn't impl Clone).
  fn get_submersion_state(&self, entity: Entity) -> Option<SubmersionStateSnapshot> {
    self
      .app
      .world()
      .get::<SubmersionState>(entity)
      .map(|s| SubmersionStateSnapshot {
        is_submerged: s.is_submerged,
        submerged_fraction: s.submerged_fraction,
        debug_liquid_samples: s.debug_liquid_samples,
        debug_total_samples: s.debug_total_samples,
      })
  }

  /// Checks if entity has the Submergent marker.
  fn has_submergent_marker(&self, entity: Entity) -> bool {
    self.app.world().get::<Submergent>(entity).is_some()
  }

  /// Reads all new Submerged messages since last read.
  fn read_submerged_messages(&mut self) -> Vec<SubmergedSnapshot> {
    let messages = self.app.world().resource::<Messages<Submerged>>();
    self
      .submerged_cursor
      .read(messages)
      .map(|m| SubmergedSnapshot {
        entity: m.entity,
        submerged_fraction: m.submerged_fraction,
      })
      .collect()
  }

  /// Reads all new Surfaced messages since last read.
  fn read_surfaced_messages(&mut self) -> Vec<SurfacedSnapshot> {
    let messages = self.app.world().resource::<Messages<Surfaced>>();
    self
      .surfaced_cursor
      .read(messages)
      .map(|m| SurfacedSnapshot { entity: m.entity })
      .collect()
  }

  /// Counts total pixel bodies.
  fn count_pixel_bodies(&mut self) -> usize {
    let mut q = self.app.world_mut().query::<&PixelBody>();
    q.iter(self.app.world()).count()
  }
}

/// Creates an 8x8 RGBA test image with all white pixels.
fn create_test_image(app: &mut App) -> Handle<Image> {
  let size = 8;

  let mut image = Image::new_fill(
    bevy::render::render_resource::Extent3d {
      width: size,
      height: size,
      depth_or_array_layers: 1,
    },
    bevy::render::render_resource::TextureDimension::D2,
    &[255, 255, 255, 255],
    bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
    RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
  );
  image.sampler = ImageSampler::nearest();

  let mut images = app.world_mut().resource_mut::<Assets<Image>>();
  images.add(image)
}

/// Tests the full submergence detection lifecycle:
/// 1. Spawn body above water - not submerged, no messages
/// 2. Teleport into water - becomes submerged, Submerged message
/// 3. Teleport out of water - surfaces, Surfaced message
#[test]
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
fn submergence_detection_lifecycle() {
  let temp_dir = TempDir::new().unwrap();
  let save_path = temp_dir.path().join("submergence_test.save");

  let mut harness = TestHarness::new(&save_path);

  // Wait for world to seed
  harness.run_until_seeded();

  // === PHASE 1: Create water pool ===
  let pool_center = WorldPos::new(0, -50);
  harness.paint_liquid_pool(pool_center, 60, 40);

  // Run a frame to process pixel changes
  harness.run(1);

  // Verify pool exists by checking a pixel
  {
    let mut world_q = harness.app.world_mut().query::<&PixelWorld>();
    let world = world_q.single(harness.app.world()).unwrap();
    let pixel = world.get_pixel(pool_center);
    assert!(
      pixel.is_some_and(|p| p.material == material_ids::WATER),
      "Water pool should exist at center"
    );
  }

  // === PHASE 2: Spawn body above pool ===
  let above_pool = Vec2::new(0.0, 50.0);
  let body_entity = harness.spawn_pixel_body(above_pool);

  assert_eq!(
    harness.count_pixel_bodies(),
    1,
    "Should have one pixel body"
  );

  // Verify body has Submergent marker (added automatically)
  assert!(
    harness.has_submergent_marker(body_entity),
    "Body should have Submergent marker"
  );

  // Run a few frames to let submersion sampling run
  harness.run(5);

  // Clear any spurious messages from initialization
  let _ = harness.read_submerged_messages();
  let _ = harness.read_surfaced_messages();

  // Check initial state - should NOT be submerged (above water)
  let state = harness.get_submersion_state(body_entity);
  assert!(state.is_some(), "Body should have SubmersionState");
  let state = state.unwrap();
  assert!(
    !state.is_submerged,
    "Body above water should not be submerged (fraction: {})",
    state.submerged_fraction
  );

  // === PHASE 3: Teleport into water ===
  let in_pool = Vec2::new(0.0, -50.0);
  harness.teleport_body(body_entity, in_pool);

  // Run frames to let submersion detection update
  let mut submerged_found = false;
  for _ in 0..5 {
    harness.run(1);
    let msgs = harness.read_submerged_messages();
    if !msgs.is_empty() {
      submerged_found = true;
      break;
    }
  }

  // Check submerged state
  let state = harness.get_submersion_state(body_entity).unwrap();
  assert!(
    state.is_submerged,
    "Body in water should be submerged (fraction: {}, liquid_samples: {}, total_samples: {})",
    state.submerged_fraction, state.debug_liquid_samples, state.debug_total_samples
  );

  // Verify Submerged message was received
  assert!(
    submerged_found,
    "Should have received Submerged message when entering water"
  );

  // === PHASE 4: Teleport out of water (surface) ===
  // Read and discard any pending messages before teleport
  let _ = harness.read_surfaced_messages();

  harness.teleport_body(body_entity, above_pool);

  // Run frames to let submersion detection update
  let mut surfaced_found = false;
  for _ in 0..5 {
    harness.run(1);
    let msgs = harness.read_surfaced_messages();
    if !msgs.is_empty() {
      surfaced_found = true;
      break;
    }
  }

  // Check surfaced state
  let state = harness.get_submersion_state(body_entity).unwrap();
  assert!(
    !state.is_submerged,
    "Body above water should not be submerged after surfacing (fraction: {})",
    state.submerged_fraction
  );

  // Check for Surfaced message
  assert!(
    surfaced_found,
    "Should have received Surfaced message during surfacing frames"
  );
}

/// Tests that submersion fraction reflects partial submersion.
#[test]
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
fn partial_submersion_tracking() {
  let temp_dir = TempDir::new().unwrap();
  let save_path = temp_dir.path().join("partial_submersion_test.save");

  let mut harness = TestHarness::new(&save_path);
  harness.run_until_seeded();

  // Create a shallow pool (body will be partially submerged)
  let pool_center = WorldPos::new(0, 0);
  harness.paint_liquid_pool(pool_center, 60, 10); // Only 10 pixels tall

  harness.run(1);

  // Spawn body at edge of pool (partially in water)
  let edge_position = Vec2::new(0.0, 5.0); // Just above pool top
  let body_entity = harness.spawn_pixel_body(edge_position);

  harness.run(5);

  let state = harness.get_submersion_state(body_entity);
  assert!(state.is_some(), "Body should have SubmersionState");

  // The body should have some fraction recorded (may or may not cross threshold)
  // This test just verifies the sampling is working
  let state = state.unwrap();
  assert!(
    state.debug_total_samples > 0,
    "Should have sampled the body (total_samples: {})",
    state.debug_total_samples
  );
}
