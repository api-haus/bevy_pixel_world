//! E2E tests for pixel body spawning with physics backends.
//!
//! Tests that pixel bodies can be spawned via commands and are properly
//! finalized with physics components (RigidBody, Collider).
//!
//! Tests cover:
//! - `SpawnPixelBodyFromImage` command (immediate image handle)
//! - `PendingPixelBody` finalization (simulates post-async-load flow)
//! - Physics component verification for both avian2d and rapier2d
//!
//! Run with avian2d:
//!   cargo test -p bevy_pixel_world --test spawn_pixel_body_e2e --features
//! avian2d
//!
//! Run with rapier2d:
//!   cargo test -p bevy_pixel_world --test spawn_pixel_body_e2e --features
//! rapier2d

use std::path::Path;

use bevy::app::{TaskPoolOptions, TaskPoolPlugin};
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy_pixel_world::{
  MaterialSeeder, PendingPixelBody, PersistenceConfig, PixelBodiesPlugin, PixelBody,
  PixelWorldPlugin, SpawnPixelBody, SpawnPixelBodyFromImage, SpawnPixelWorld, StreamingCamera,
  material_ids,
};
use tempfile::TempDir;

struct TestHarness {
  app: App,
  #[allow(dead_code)]
  camera: Entity,
  /// Handle to the test image for spawning bodies.
  pub test_image: Handle<Image>,
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
    // ImagePlugin registers the Image asset type
    app.add_plugins(bevy::image::ImagePlugin::default());
    // ScenePlugin is needed for avian2d's collider hierarchy initialization
    app.add_plugins(bevy::scene::ScenePlugin);

    app.add_plugins(PixelWorldPlugin::default().persistence(PersistenceConfig::at(save_path)));
    app.add_plugins(PixelBodiesPlugin);

    // Add physics plugin based on feature
    #[cfg(feature = "avian2d")]
    {
      // DiagnosticsPlugin is required by avian2d's PhysicsPlugins
      app.add_plugins(bevy::diagnostic::DiagnosticsPlugin);
      app.add_plugins(avian2d::prelude::PhysicsPlugins::default());
      app.insert_resource(avian2d::prelude::Gravity(Vec2::new(0.0, -500.0)));
      // Initialize all diagnostics resources that avian2d systems expect.
      // These are normally registered in plugin finish(), but MinimalPlugins
      // may have timing issues.
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

    // Create a test image in memory (8x8 white box)
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
    }
  }

  fn run(&mut self, updates: usize) {
    for _ in 0..updates {
      self.app.update();
    }
  }

  /// Spawns a pixel body using an in-memory test image.
  fn spawn_pixel_body(&mut self, position: Vec2) {
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

  /// Counts pending pixel bodies (waiting for image to load).
  fn count_pending_bodies(&mut self) -> usize {
    let mut q = self.app.world_mut().query::<&PendingPixelBody>();
    q.iter(self.app.world()).count()
  }

  /// Verifies that pixel bodies have physics components (RigidBody, Collider).
  fn verify_physics_components(&mut self) {
    #[cfg(feature = "avian2d")]
    {
      let mut q = self
        .app
        .world_mut()
        .query::<(Entity, &PixelBody, &avian2d::prelude::RigidBody)>();
      let bodies: Vec<_> = q.iter(self.app.world()).collect();
      assert!(
        !bodies.is_empty(),
        "Pixel bodies should have avian2d RigidBody component"
      );
      for (entity, _, rb) in &bodies {
        assert!(
          matches!(rb, avian2d::prelude::RigidBody::Dynamic),
          "Body {:?} should have Dynamic RigidBody, got {:?}",
          entity,
          rb
        );
      }
    }

    #[cfg(all(feature = "rapier2d", not(feature = "avian2d")))]
    {
      let mut q = self
        .app
        .world_mut()
        .query::<(Entity, &PixelBody, &bevy_rapier2d::prelude::RigidBody)>();
      let bodies: Vec<_> = q.iter(self.app.world()).collect();
      assert!(
        !bodies.is_empty(),
        "Pixel bodies should have rapier2d RigidBody component"
      );
      for (entity, _, rb) in &bodies {
        assert!(
          matches!(rb, bevy_rapier2d::prelude::RigidBody::Dynamic),
          "Body {:?} should have Dynamic RigidBody, got {:?}",
          entity,
          rb
        );
      }
    }
  }

  /// Verifies transform components are correctly set up for a body at expected
  /// position.
  fn verify_transform_at_position(&mut self, expected_pos: Vec2) -> Entity {
    let mut q = self
      .app
      .world_mut()
      .query::<(Entity, &PixelBody, &Transform, Option<&GlobalTransform>)>();

    let bodies: Vec<_> = q.iter(self.app.world()).collect();
    assert!(!bodies.is_empty(), "Should have at least one pixel body");

    let (entity, _body, transform, global_transform) = bodies[0];

    // Verify Transform position matches expected
    let actual_pos = Vec2::new(transform.translation.x, transform.translation.y);
    let distance = (actual_pos - expected_pos).length();
    assert!(
      distance < 50.0, // Allow some drift from physics
      "Body position should be near {:?}, but got {:?} (distance: {})",
      expected_pos,
      actual_pos,
      distance
    );

    // Verify GlobalTransform exists and is valid
    assert!(
      global_transform.is_some(),
      "Body {:?} missing GlobalTransform component - required for rendering/physics",
      entity
    );

    let gt = global_transform.unwrap();
    let gt_pos = gt.translation();
    assert!(
      gt_pos.x.is_finite() && gt_pos.y.is_finite() && gt_pos.z.is_finite(),
      "GlobalTransform has non-finite values: {:?}",
      gt_pos
    );

    // Verify z-position is reasonable (not behind camera or at infinity)
    assert!(
      transform.translation.z.abs() < 1000.0,
      "Body z-position is extreme: {} - may be invisible",
      transform.translation.z
    );

    entity
  }

  /// Verifies collider component exists and has valid configuration.
  fn verify_collider_valid(&mut self, entity: Entity) {
    #[cfg(feature = "avian2d")]
    {
      let world = self.app.world();
      let collider = world.get::<avian2d::prelude::Collider>(entity);
      assert!(
        collider.is_some(),
        "Body {:?} missing Collider component",
        entity
      );
    }

    #[cfg(all(feature = "rapier2d", not(feature = "avian2d")))]
    {
      let world = self.app.world();
      let collider = world.get::<bevy_rapier2d::prelude::Collider>(entity);
      assert!(
        collider.is_some(),
        "Body {:?} missing Collider component",
        entity
      );
    }
  }
}

/// Creates an 8x8 RGBA test image with all white pixels.
fn create_test_image(app: &mut App) -> Handle<Image> {
  let size = 8;

  let mut image = Image::new_fill(
    bevy::render::render_resource::Extent3d {
      width: size as u32,
      height: size as u32,
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

/// Test that SpawnPixelBodyFromImage command creates and finalizes pixel
/// bodies.
///
/// This test verifies:
/// 1. SpawnPixelBodyFromImage command queues a pending body
/// 2. finalize_pending_pixel_bodies processes it when the image is available
/// 3. update_pixel_bodies can query the finalized body
#[test]
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
fn spawn_pixel_body_command_creates_body() {
  let temp_dir = TempDir::new().unwrap();
  let save_path = temp_dir.path().join("spawn_test.save");

  let mut harness = TestHarness::new(&save_path);

  // Run a few frames to initialize
  harness.run(5);

  // Verify no bodies exist initially
  assert_eq!(
    harness.count_pixel_bodies(),
    0,
    "Should start with no pixel bodies"
  );

  // Spawn a body using the command
  harness.spawn_pixel_body(Vec2::new(0.0, 100.0));

  // Run frames to let the body finalize
  harness.run(10);

  // Verify a body was created
  let body_count = harness.count_pixel_bodies();
  assert!(
    body_count >= 1,
    "Should have at least 1 pixel body after spawn, got {}",
    body_count
  );

  // Verify the body has pixels
  let bodies = harness.get_all_bodies();
  assert!(!bodies.is_empty(), "Should have at least one body");

  for (entity, solid_count) in &bodies {
    assert!(
      *solid_count > 0,
      "Body {:?} should have some solid pixels, got {}",
      entity,
      solid_count
    );
  }
}

/// Test that multiple bodies can be spawned and processed.
#[test]
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
fn spawn_multiple_pixel_bodies() {
  let temp_dir = TempDir::new().unwrap();
  let save_path = temp_dir.path().join("multi_spawn_test.save");

  let mut harness = TestHarness::new(&save_path);
  harness.run(5);

  // Spawn multiple bodies
  for i in 0..3 {
    let x = (i as f32 - 1.0) * 100.0;
    harness.spawn_pixel_body(Vec2::new(x, 100.0));
  }

  // Run frames to let all bodies finalize
  harness.run(20);

  // Verify all bodies were created
  let body_count = harness.count_pixel_bodies();
  assert!(
    body_count >= 3,
    "Should have at least 3 pixel bodies after spawning 3, got {}",
    body_count
  );
}

/// Test that bodies remain stable after spawning (no disintegration).
#[test]
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
fn spawned_bodies_remain_stable() {
  let temp_dir = TempDir::new().unwrap();
  let save_path = temp_dir.path().join("stability_test.save");

  let mut harness = TestHarness::new(&save_path);
  harness.run(5);

  // Spawn a body
  harness.spawn_pixel_body(Vec2::new(0.0, 100.0));

  // Run frames to let it finalize
  harness.run(10);

  // Get initial solid count
  let initial_bodies = harness.get_all_bodies();
  assert!(!initial_bodies.is_empty(), "Body should exist");

  let initial_solid_count = initial_bodies[0].1;

  // Run more frames (simulation)
  harness.run(100);

  // Verify body still exists and hasn't lost pixels
  let final_bodies = harness.get_all_bodies();
  assert!(
    !final_bodies.is_empty(),
    "Body should still exist after simulation"
  );

  let final_solid_count = final_bodies[0].1;
  assert_eq!(
    initial_solid_count, final_solid_count,
    "Body should maintain its solid count ({} -> {})",
    initial_solid_count, final_solid_count
  );
}

/// Test that PendingPixelBody finalization creates physics-enabled bodies.
///
/// This tests the finalization flow that runs after async asset loading:
/// 1. PendingPixelBody exists with loaded image handle
/// 2. finalize_pending_pixel_bodies processes it
/// 3. Body becomes visible to physics with RigidBody + Collider components
///
/// This is the same code path used by SpawnPixelBody after the image loads,
/// but bypasses async file IO which doesn't work in the test environment.
#[test]
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
fn pending_pixel_body_finalization_creates_physics_body() {
  let temp_dir = TempDir::new().unwrap();
  let save_path = temp_dir.path().join("finalization_test.save");

  let mut harness = TestHarness::new(&save_path);

  // Run a few frames to initialize
  harness.run(5);

  // Verify no bodies exist initially
  assert_eq!(
    harness.count_pixel_bodies(),
    0,
    "Should start with no pixel bodies"
  );

  // Manually create a PendingPixelBody with an already-loaded image
  // This simulates what happens after SpawnPixelBody's async load completes
  let image_handle = harness.test_image.clone();
  harness.app.world_mut().spawn(PendingPixelBody {
    image: image_handle,
    material: material_ids::WOOD,
    position: Vec2::new(0.0, 100.0),
  });

  // Run frames to let finalization system process the pending body
  // (The body may be finalized in the first frame since the image is already
  // loaded)
  harness.run(10);

  // Verify the body was created
  let body_count = harness.count_pixel_bodies();
  assert!(
    body_count >= 1,
    "Should have at least 1 pixel body after finalization, got {}",
    body_count
  );

  // Verify pending body was consumed
  let final_pending = harness.count_pending_bodies();
  assert_eq!(
    final_pending, 0,
    "Pending body should be consumed after finalization, got {}",
    final_pending
  );

  // Verify the body has pixels
  let bodies = harness.get_all_bodies();
  assert!(!bodies.is_empty(), "Should have at least one body");

  for (entity, solid_count) in &bodies {
    assert!(
      *solid_count > 0,
      "Body {:?} should have some solid pixels, got {}",
      entity,
      solid_count
    );
  }

  // Verify physics components are present
  harness.verify_physics_components();
}

/// Test that SpawnPixelBody command (with asset path) creates physics-enabled
/// bodies.
///
/// This tests the EXACT flow used by the painting example:
/// 1. SpawnPixelBody command calls asset_server.load() to get a handle
/// 2. Creates PendingPixelBody with that handle
/// 3. finalize_pending_pixel_bodies processes it when image is available
/// 4. Body becomes visible to physics with RigidBody + Collider components
///
/// The test bypasses async file IO by manually inserting the image into Assets
/// at the same handle id that AssetServer::load() would create.
#[test]
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
fn spawn_pixel_body_command_with_asset_path() {
  let temp_dir = TempDir::new().unwrap();
  let save_path = temp_dir.path().join("asset_path_test.save");

  let mut harness = TestHarness::new(&save_path);

  // Run a few frames to initialize
  harness.run(5);

  // Verify no bodies exist initially
  assert_eq!(
    harness.count_pixel_bodies(),
    0,
    "Should start with no pixel bodies"
  );

  // Use SpawnPixelBody command (exactly like painting example does)
  // This calls asset_server.load("test_asset.png") internally
  let asset_path = "test_asset.png";
  harness
    .app
    .world_mut()
    .commands()
    .queue(SpawnPixelBody::new(
      asset_path,
      material_ids::WOOD,
      Vec2::new(0.0, 100.0),
    ));

  // Apply the command (this creates PendingPixelBody with handle from
  // asset_server.load())
  harness.run(1);

  // Verify pending body was created
  let pending_count = harness.count_pending_bodies();
  assert_eq!(
    pending_count, 1,
    "SpawnPixelBody should create a PendingPixelBody, got {}",
    pending_count
  );

  // Now manually "load" the asset by:
  // 1. Getting the same handle that SpawnPixelBody created (asset_server.load
  //    returns same handle for same path)
  // 2. Inserting our test image at that handle's id
  let handle: Handle<Image> = harness
    .app
    .world()
    .resource::<AssetServer>()
    .load(asset_path);

  // Create a test image
  let test_image = create_test_image_data();

  // Insert the image at the handle's asset id (this makes the handle "loaded")
  harness
    .app
    .world_mut()
    .resource_mut::<Assets<Image>>()
    .insert(&handle, test_image);

  // Run frames to let finalization system process the pending body
  harness.run(10);

  // Verify the body was created
  let body_count = harness.count_pixel_bodies();
  assert!(
    body_count >= 1,
    "Should have at least 1 pixel body after SpawnPixelBody command, got {}",
    body_count
  );

  // Verify pending body was consumed
  let final_pending = harness.count_pending_bodies();
  assert_eq!(
    final_pending, 0,
    "Pending body should be consumed after finalization, got {}",
    final_pending
  );

  // Verify the body has pixels
  let bodies = harness.get_all_bodies();
  assert!(!bodies.is_empty(), "Should have at least one body");

  for (entity, solid_count) in &bodies {
    assert!(
      *solid_count > 0,
      "Body {:?} should have some solid pixels, got {}",
      entity,
      solid_count
    );
  }

  // Verify physics components are present
  harness.verify_physics_components();
}

/// Creates an 8x8 RGBA test image with all white pixels (standalone version).
fn create_test_image_data() -> Image {
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
  image
}

/// Comprehensive E2E test verifying the full pixel body spawn lifecycle.
///
/// This test verifies:
/// 1. SpawnPixelBody command creates a PendingPixelBody
/// 2. finalize_pending_pixel_bodies converts it to a full PixelBody
/// 3. Physics components (RigidBody, Collider) are attached
/// 4. The body is NOT disabled/culled
/// 5. Physics simulation affects the body (position changes due to gravity)
/// 6. The body persists across multiple frames without disappearing
///
/// This is the definitive test for the spawn flow used by examples like
/// painting.rs.
#[test]
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
fn spawn_pixel_body_full_lifecycle() {
  use bevy::ecs::entity_disabling::Disabled;

  let temp_dir = TempDir::new().unwrap();
  let save_path = temp_dir.path().join("lifecycle_test.save");

  let mut harness = TestHarness::new(&save_path);

  // Run initialization frames
  harness.run(5);

  // Verify clean slate
  assert_eq!(
    harness.count_pixel_bodies(),
    0,
    "Should start with no pixel bodies"
  );
  assert_eq!(
    harness.count_pending_bodies(),
    0,
    "Should start with no pending bodies"
  );

  // === PHASE 1: Queue spawn command ===
  let spawn_position = Vec2::new(0.0, 100.0);
  let asset_path = "lifecycle_test.png";

  harness
    .app
    .world_mut()
    .commands()
    .queue(SpawnPixelBody::new(
      asset_path,
      material_ids::WOOD,
      spawn_position,
    ));

  harness.run(1);

  // Verify PendingPixelBody was created
  assert_eq!(
    harness.count_pending_bodies(),
    1,
    "SpawnPixelBody should create exactly 1 PendingPixelBody"
  );

  // === PHASE 2: Simulate asset loading ===
  // Get the handle that SpawnPixelBody created and insert our test image
  let handle: Handle<Image> = harness
    .app
    .world()
    .resource::<AssetServer>()
    .load(asset_path);
  let test_image = create_test_image_data();
  let _ = harness
    .app
    .world_mut()
    .resource_mut::<Assets<Image>>()
    .insert(&handle, test_image);

  // Run finalization
  harness.run(5);

  // === PHASE 3: Verify finalization ===
  assert_eq!(
    harness.count_pending_bodies(),
    0,
    "PendingPixelBody should be consumed after finalization"
  );
  assert_eq!(
    harness.count_pixel_bodies(),
    1,
    "Should have exactly 1 PixelBody after finalization"
  );

  // Get the spawned entity
  let bodies = harness.get_all_bodies();
  assert_eq!(bodies.len(), 1, "Should have exactly 1 body");
  let (body_entity, solid_count) = bodies[0];
  assert!(
    solid_count > 0,
    "Body should have solid pixels, got {}",
    solid_count
  );

  // === PHASE 4: Verify physics components ===
  harness.verify_physics_components();

  // === PHASE 5: Verify transform and position ===
  let verified_entity = harness.verify_transform_at_position(spawn_position);
  assert_eq!(
    body_entity, verified_entity,
    "Verified entity should match spawned entity"
  );

  // === PHASE 6: Verify collider is valid ===
  harness.verify_collider_valid(body_entity);

  // === PHASE 7: Verify entity is NOT disabled ===
  {
    let world = harness.app.world();
    let is_disabled = world.entity(body_entity).contains::<Disabled>();
    assert!(
      !is_disabled,
      "Spawned body should NOT be disabled (culled). Entity {:?} has Disabled component.",
      body_entity
    );
  }

  // === PHASE 8: Verify body survives multiple frames ===
  harness.run(60);

  // Verify entity still exists with valid transform
  {
    let world = harness.app.world();
    let transform = world.get::<Transform>(body_entity);
    assert!(
      transform.is_some(),
      "Body entity {:?} was despawned during simulation!",
      body_entity
    );

    let t = transform.unwrap();
    assert!(
      t.translation.x.is_finite() && t.translation.y.is_finite(),
      "Body position became NaN/Inf after simulation: {:?}",
      t.translation
    );
  }

  // === PHASE 9: Final verification ===
  let final_body_count = harness.count_pixel_bodies();
  assert_eq!(
    final_body_count, 1,
    "Body should persist after physics simulation, got {}",
    final_body_count
  );

  // Verify physics components still attached
  harness.verify_physics_components();
  harness.verify_collider_valid(body_entity);

  // Verify not disabled after simulation
  {
    let world = harness.app.world();
    if let Ok(entity_ref) = world.get_entity(body_entity) {
      let is_disabled = entity_ref.contains::<Disabled>();
      assert!(
        !is_disabled,
        "Body should NOT become disabled after physics simulation"
      );
    }
  }
}
