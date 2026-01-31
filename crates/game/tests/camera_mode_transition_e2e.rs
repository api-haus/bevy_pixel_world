//! E2E test for camera projection stability during mode transitions.
//!
//! Tests the bug where exiting play mode causes a zoom-in effect because
//! `update_camera_on_config_change` was modifying ALL Camera2d projections
//! instead of just the GameCamera.

use bevy::camera::ScalingMode;
use bevy::prelude::*;

/// Marker for the game camera (like in the actual game)
#[derive(Component)]
struct TestGameCamera;

/// Marker for a secondary camera (like PixelBlitCamera/PixelFullresCamera)
#[derive(Component)]
struct TestSecondaryCamera;

/// Marker for the pixel scene camera (simulates PixelSceneCamera from bevy_pixel_world)
/// This is added to the GameCamera entity when pixel camera is initialized.
#[derive(Component)]
struct TestPixelSceneCamera;

/// Config resource that can change
#[derive(Resource, Clone)]
struct TestConfig {
  viewport_width: f32,
  viewport_height: f32,
}

impl Default for TestConfig {
  fn default() -> Self {
    Self {
      viewport_width: 640.0,
      viewport_height: 480.0,
    }
  }
}

/// System that updates camera on config change.
/// CORRECT: Only targets TestGameCamera, excludes TestPixelSceneCamera
fn update_camera_on_config_change_correct(
  config: Res<TestConfig>,
  mut camera_query: Query<&mut Projection, (With<TestGameCamera>, Without<TestPixelSceneCamera>)>,
) {
  if config.is_changed() {
    for mut projection in camera_query.iter_mut() {
      if let Projection::Orthographic(ref mut ortho) = *projection {
        ortho.scaling_mode = ScalingMode::AutoMin {
          min_width: config.viewport_width,
          min_height: config.viewport_height,
        };
      }
    }
  }
}

/// System that updates camera on config change.
/// PARTIALLY FIXED: Targets GameCamera but doesn't exclude PixelSceneCamera
fn update_camera_on_config_change_partial_fix(
  config: Res<TestConfig>,
  mut camera_query: Query<&mut Projection, With<TestGameCamera>>,
) {
  if config.is_changed() {
    for mut projection in camera_query.iter_mut() {
      if let Projection::Orthographic(ref mut ortho) = *projection {
        ortho.scaling_mode = ScalingMode::AutoMin {
          min_width: config.viewport_width,
          min_height: config.viewport_height,
        };
      }
    }
  }
}

/// System that updates camera on config change.
/// BUGGY: Targets ALL Camera2d entities
fn update_camera_on_config_change_buggy(
  config: Res<TestConfig>,
  mut camera_query: Query<&mut Projection, With<Camera2d>>,
) {
  if config.is_changed() {
    for mut projection in camera_query.iter_mut() {
      if let Projection::Orthographic(ref mut ortho) = *projection {
        ortho.scaling_mode = ScalingMode::AutoMin {
          min_width: config.viewport_width,
          min_height: config.viewport_height,
        };
      }
    }
  }
}

/// Helper to check if scaling mode is Fixed with expected dimensions
fn is_fixed_scaling(projection: &Projection, expected_width: f32, expected_height: f32) -> bool {
  match projection {
    Projection::Orthographic(ortho) => {
      matches!(
        ortho.scaling_mode,
        ScalingMode::Fixed { width, height } if (width - expected_width).abs() < 0.01 && (height - expected_height).abs() < 0.01
      )
    }
    _ => false,
  }
}

/// Helper to check if scaling mode is AutoMin
fn is_auto_min_scaling(projection: &Projection) -> bool {
  match projection {
    Projection::Orthographic(ortho) => {
      matches!(ortho.scaling_mode, ScalingMode::AutoMin { .. })
    }
    _ => false,
  }
}

/// Helper to get AutoMin dimensions if present
fn get_auto_min_width(projection: &Projection) -> Option<f32> {
  match projection {
    Projection::Orthographic(ortho) => match ortho.scaling_mode {
      ScalingMode::AutoMin { min_width, .. } => Some(min_width),
      _ => None,
    },
    _ => None,
  }
}

/// Debug helper to describe scaling mode
fn describe_scaling_mode(projection: &Projection) -> String {
  match projection {
    Projection::Orthographic(ortho) => match &ortho.scaling_mode {
      ScalingMode::Fixed { width, height } => format!("Fixed({}, {})", width, height),
      ScalingMode::AutoMin { min_width, min_height } => {
        format!("AutoMin({}, {})", min_width, min_height)
      }
      ScalingMode::AutoMax { max_width, max_height } => {
        format!("AutoMax({}, {})", max_width, max_height)
      }
      other => format!("{:?}", other),
    },
    _ => "Perspective".to_string(),
  }
}

/// Test that verifies the CORRECT behavior: only GameCamera is affected
#[test]
fn test_config_change_only_affects_game_camera() {
  let mut app = App::new();
  app.add_plugins(MinimalPlugins);

  // Insert initial config
  app.insert_resource(TestConfig::default());

  // Add the CORRECT system
  app.add_systems(Update, update_camera_on_config_change_correct);

  // Spawn game camera with AutoMin
  let game_camera = app
    .world_mut()
    .spawn((
      TestGameCamera,
      Camera2d,
      Projection::Orthographic(OrthographicProjection {
        scaling_mode: ScalingMode::AutoMin {
          min_width: 640.0,
          min_height: 480.0,
        },
        ..OrthographicProjection::default_2d()
      }),
    ))
    .id();

  // Spawn secondary camera with Fixed (like PixelBlitCamera)
  let secondary_camera = app
    .world_mut()
    .spawn((
      TestSecondaryCamera,
      Camera2d,
      Projection::Orthographic(OrthographicProjection {
        scaling_mode: ScalingMode::Fixed {
          width: 2.0,
          height: 2.0,
        },
        ..OrthographicProjection::default_2d()
      }),
    ))
    .id();

  // Run initial update (config will be marked as changed on first run)
  app.update();

  // Verify initial state after first update
  let game_proj = app.world().get::<Projection>(game_camera).unwrap();
  let secondary_proj = app.world().get::<Projection>(secondary_camera).unwrap();

  // Game camera should have AutoMin (might be updated by the system)
  assert!(
    is_auto_min_scaling(game_proj),
    "Game camera should have AutoMin scaling"
  );

  // Secondary camera should STILL have Fixed
  assert!(
    is_fixed_scaling(secondary_proj, 2.0, 2.0),
    "Secondary camera should still have Fixed scaling, got {}",
    describe_scaling_mode(secondary_proj)
  );

  // Now change the config
  app.world_mut().resource_mut::<TestConfig>().viewport_width = 800.0;
  app.update();

  // Verify game camera was updated
  let game_proj = app.world().get::<Projection>(game_camera).unwrap();
  let new_width = get_auto_min_width(game_proj);
  assert_eq!(
    new_width,
    Some(800.0),
    "Game camera should be updated to new width"
  );

  // Verify secondary camera was NOT updated
  let secondary_proj = app.world().get::<Projection>(secondary_camera).unwrap();
  assert!(
    is_fixed_scaling(secondary_proj, 2.0, 2.0),
    "Secondary camera should STILL have Fixed scaling after config change, got {}",
    describe_scaling_mode(secondary_proj)
  );

  println!("PASS: Correct implementation only affects GameCamera");
}

/// Test that demonstrates the BUG: ALL Camera2d entities are affected
#[test]
fn test_buggy_config_change_affects_all_cameras() {
  let mut app = App::new();
  app.add_plugins(MinimalPlugins);

  // Insert initial config
  app.insert_resource(TestConfig::default());

  // Add the BUGGY system
  app.add_systems(Update, update_camera_on_config_change_buggy);

  // Spawn game camera with AutoMin
  let game_camera = app
    .world_mut()
    .spawn((
      TestGameCamera,
      Camera2d,
      Projection::Orthographic(OrthographicProjection {
        scaling_mode: ScalingMode::AutoMin {
          min_width: 640.0,
          min_height: 480.0,
        },
        ..OrthographicProjection::default_2d()
      }),
    ))
    .id();

  // Spawn secondary camera with Fixed (like PixelBlitCamera)
  let secondary_camera = app
    .world_mut()
    .spawn((
      TestSecondaryCamera,
      Camera2d,
      Projection::Orthographic(OrthographicProjection {
        scaling_mode: ScalingMode::Fixed {
          width: 2.0,
          height: 2.0,
        },
        ..OrthographicProjection::default_2d()
      }),
    ))
    .id();

  // Run initial update (config will be marked as changed on first run)
  app.update();

  // Verify game camera has AutoMin
  let game_proj = app.world().get::<Projection>(game_camera).unwrap();
  assert!(
    is_auto_min_scaling(game_proj),
    "Game camera should have AutoMin"
  );

  // BUG: Secondary camera SHOULD have Fixed, but buggy system overwrites it
  let secondary_proj = app.world().get::<Projection>(secondary_camera).unwrap();

  // This test EXPECTS the bug to exist (demonstrating the problem)
  assert!(
    is_auto_min_scaling(secondary_proj),
    "BUG DEMO: Secondary camera was incorrectly changed to AutoMin. Got {}",
    describe_scaling_mode(secondary_proj)
  );

  println!("PASS: Buggy implementation affects ALL Camera2d (demonstrating the bug)");
}

/// Test simulating mode transition with config resource reinsertion
#[test]
fn test_mode_transition_config_stability() {
  let mut app = App::new();
  app.add_plugins(MinimalPlugins);

  // Insert initial config
  app.insert_resource(TestConfig::default());

  // Add the CORRECT system
  app.add_systems(Update, update_camera_on_config_change_correct);

  // Spawn cameras
  let game_camera = app
    .world_mut()
    .spawn((
      TestGameCamera,
      Camera2d,
      Projection::Orthographic(OrthographicProjection {
        scaling_mode: ScalingMode::AutoMin {
          min_width: 640.0,
          min_height: 480.0,
        },
        ..OrthographicProjection::default_2d()
      }),
    ))
    .id();

  let blit_camera = app
    .world_mut()
    .spawn((
      TestSecondaryCamera,
      Camera2d,
      Projection::Orthographic(OrthographicProjection {
        scaling_mode: ScalingMode::Fixed {
          width: 2.0,
          height: 2.0,
        },
        ..OrthographicProjection::default_2d()
      }),
    ))
    .id();

  // Initial update
  app.update();

  // Record initial state - blit camera should be Fixed 2x2
  let blit_proj = app.world().get::<Projection>(blit_camera).unwrap();
  println!(
    "Initial blit camera scaling: {}",
    describe_scaling_mode(blit_proj)
  );
  assert!(
    is_fixed_scaling(blit_proj, 2.0, 2.0),
    "Initial blit camera should be Fixed(2,2)"
  );

  // Simulate "entering play mode" - no config change expected
  app.update();
  app.update();

  // Simulate "exiting play mode" - this is where the bug manifested
  // In real game, config.is_changed() might trigger due to resource system behavior
  // Simulate this by re-inserting the config (which marks it as changed)
  let current_config = app.world().resource::<TestConfig>().clone();
  app.world_mut().insert_resource(current_config);
  app.update();

  // Check blit camera wasn't affected
  let final_blit_proj = app.world().get::<Projection>(blit_camera).unwrap();
  println!(
    "Final blit camera scaling: {}",
    describe_scaling_mode(final_blit_proj)
  );

  assert!(
    is_fixed_scaling(final_blit_proj, 2.0, 2.0),
    "Blit camera scaling should NOT change during mode transitions, got {}",
    describe_scaling_mode(final_blit_proj)
  );

  // Also verify game camera is still correct
  let game_proj = app.world().get::<Projection>(game_camera).unwrap();
  assert!(
    is_auto_min_scaling(game_proj),
    "Game camera should still have AutoMin"
  );

  println!("PASS: Mode transition doesn't affect secondary cameras");
}

/// Test that simulates the ACTUAL pixel camera scenario:
/// 1. GameCamera spawned with AutoMin
/// 2. Pixel camera system adds PixelSceneCamera and changes to Fixed
/// 3. Config change occurs
/// 4. Projection should NOT be modified because PixelSceneCamera is excluded
#[test]
fn test_pixel_scene_camera_excluded_from_config_updates() {
  let mut app = App::new();
  app.add_plugins(MinimalPlugins);

  // Insert initial config
  app.insert_resource(TestConfig::default());

  // Add the CORRECT system (excludes PixelSceneCamera)
  app.add_systems(Update, update_camera_on_config_change_correct);

  // Spawn game camera - this simulates the actual game setup
  // Initially has GameCamera + AutoMin, then pixel camera system adds PixelSceneCamera + Fixed
  let game_camera = app
    .world_mut()
    .spawn((
      TestGameCamera,
      TestPixelSceneCamera, // Added by pixel camera setup
      Camera2d,
      Projection::Orthographic(OrthographicProjection {
        // Pixel camera system changes this to Fixed
        scaling_mode: ScalingMode::Fixed {
          width: 320.0,
          height: 240.0,
        },
        ..OrthographicProjection::default_2d()
      }),
    ))
    .id();

  // Initial update
  app.update();

  // Camera should still be Fixed (excluded from config update)
  let proj = app.world().get::<Projection>(game_camera).unwrap();
  assert!(
    is_fixed_scaling(proj, 320.0, 240.0),
    "Camera with PixelSceneCamera should NOT be updated to AutoMin, got {}",
    describe_scaling_mode(proj)
  );

  // Now change the config
  app.world_mut().resource_mut::<TestConfig>().viewport_width = 800.0;
  app.update();

  // Camera should STILL be Fixed (not affected by config change)
  let proj = app.world().get::<Projection>(game_camera).unwrap();
  assert!(
    is_fixed_scaling(proj, 320.0, 240.0),
    "Camera with PixelSceneCamera should remain Fixed after config change, got {}",
    describe_scaling_mode(proj)
  );

  println!("PASS: PixelSceneCamera correctly excluded from config updates");
}

/// Test that demonstrates the PARTIAL FIX bug:
/// Targeting just GameCamera (without excluding PixelSceneCamera) still breaks
#[test]
fn test_partial_fix_still_breaks_pixel_camera() {
  let mut app = App::new();
  app.add_plugins(MinimalPlugins);

  // Insert initial config
  app.insert_resource(TestConfig::default());

  // Add the PARTIAL FIX system (targets GameCamera, doesn't exclude PixelSceneCamera)
  app.add_systems(Update, update_camera_on_config_change_partial_fix);

  // Spawn camera with both GameCamera AND PixelSceneCamera (like real pixel camera setup)
  let game_camera = app
    .world_mut()
    .spawn((
      TestGameCamera,
      TestPixelSceneCamera,
      Camera2d,
      Projection::Orthographic(OrthographicProjection {
        scaling_mode: ScalingMode::Fixed {
          width: 320.0,
          height: 240.0,
        },
        ..OrthographicProjection::default_2d()
      }),
    ))
    .id();

  // Initial update - config is changed
  app.update();

  // BUG: The partial fix still matches this camera (has GameCamera) and changes it to AutoMin
  let proj = app.world().get::<Projection>(game_camera).unwrap();
  assert!(
    is_auto_min_scaling(proj),
    "PARTIAL FIX BUG: Camera was incorrectly changed to AutoMin. Got {}",
    describe_scaling_mode(proj)
  );

  println!("PASS: Partial fix demonstrated - still breaks pixel camera");
}
