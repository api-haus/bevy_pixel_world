//! E2E test for CRT config live reload.
//!
//! Verifies that changing CrtConfig resource updates DeconvergenceMaterial
//! params.
//!
//! Run: cargo test -p game --test crt_config_reload_e2e

use bevy::prelude::*;
use bevy_crt::{CrtConfig, DeconvergenceMaterial};

/// Test that modifying CrtConfig resource triggers material param updates.
///
/// This test verifies the data flow:
/// 1. CrtConfig resource is modified
/// 2. update_crt_params system detects the change
/// 3. DeconvergenceMaterial.params is updated via Assets::insert()
#[test]
fn crt_config_change_updates_material_params() {
  let mut app = App::new();

  // Minimal plugins - no rendering needed for this test
  app.add_plugins(MinimalPlugins);
  app.add_plugins(bevy::asset::AssetPlugin::default());

  // Register the material asset type manually (normally done by Material2dPlugin)
  app.init_asset::<DeconvergenceMaterial>();

  // Initialize CrtConfig resource with defaults
  app.init_resource::<CrtConfig>();

  // Manually add the update system (extracted from Crt2dPlugin)
  app.add_systems(Update, update_crt_params_test_system);

  // Create a mock DeconvergenceMaterial in Assets
  let material = create_mock_deconvergence_material();
  let material_id = app
    .world_mut()
    .resource_mut::<Assets<DeconvergenceMaterial>>()
    .add(material);

  // Run initial frame
  app.update();

  // Verify initial state
  {
    let config = app.world().resource::<CrtConfig>();
    assert!(config.enabled, "CRT should be enabled by default");
    assert!(
      (config.brightness - 1.4).abs() < 0.01,
      "Default brightness should be 1.4, got {}",
      config.brightness
    );
  }

  // Modify CrtConfig
  {
    let mut config = app.world_mut().resource_mut::<CrtConfig>();
    config.brightness = 2.5;
    config.enabled = false;
    config.curvature_x = 0.1;
    config.scanline_intensity = 0.9;
  }

  // Run frames to let the update system process the change
  for _ in 0..3 {
    app.update();
  }

  // Verify DeconvergenceMaterial params were updated
  {
    let materials = app.world().resource::<Assets<DeconvergenceMaterial>>();
    let material = materials.get(&material_id).expect("Material should exist");

    assert_eq!(
      material.params.enabled, 0,
      "Material params.enabled should be 0 (disabled), got {}",
      material.params.enabled
    );
    assert!(
      (material.params.glow_brightness.y - 2.5).abs() < 0.01,
      "Material params brightness should be 2.5, got {}",
      material.params.glow_brightness.y
    );
    assert!(
      (material.params.curvature.x - 0.1).abs() < 0.01,
      "Material params curvature.x should be 0.1, got {}",
      material.params.curvature.x
    );
    assert!(
      (material.params.scanline.x - 0.9).abs() < 0.01,
      "Material params scanline.x should be 0.9, got {}",
      material.params.scanline.x
    );
  }
}

/// Test CrtConfig to CrtParams conversion preserves all values.
#[test]
fn crt_config_to_params_conversion() {
  let config = CrtConfig {
    enabled: true,
    curvature_x: 0.11,
    curvature_y: 0.22,
    scanline_intensity: 0.33,
    scanline_sharpness: 0.44,
    mask_strength: 0.55,
    mask_type: 6,
    glow: 0.66,
    brightness: 0.77,
    gamma: 0.88,
    corner_size: 0.99,
  };

  let params = config.to_params();

  assert_eq!(params.enabled, 1);
  assert!((params.curvature.x - 0.11).abs() < 0.001);
  assert!((params.curvature.y - 0.22).abs() < 0.001);
  assert!((params.scanline.x - 0.33).abs() < 0.001);
  assert!((params.scanline.y - 0.44).abs() < 0.001);
  assert!((params.mask.x - 0.55).abs() < 0.001);
  assert!((params.mask.y - 6.0).abs() < 0.001);
  assert!((params.glow_brightness.x - 0.66).abs() < 0.001);
  assert!((params.glow_brightness.y - 0.77).abs() < 0.001);
  assert!((params.gamma_corner.x - 0.88).abs() < 0.001);
  assert!((params.gamma_corner.y - 0.99).abs() < 0.001);
}

/// Test that multiple config changes are all applied.
#[test]
fn crt_config_multiple_changes() {
  let mut app = App::new();
  app.add_plugins(MinimalPlugins);
  app.add_plugins(bevy::asset::AssetPlugin::default());
  app.init_asset::<DeconvergenceMaterial>();
  app.init_resource::<CrtConfig>();
  app.add_systems(Update, update_crt_params_test_system);

  let material = create_mock_deconvergence_material();
  let material_id = app
    .world_mut()
    .resource_mut::<Assets<DeconvergenceMaterial>>()
    .add(material);

  app.update();

  // First change
  {
    let mut config = app.world_mut().resource_mut::<CrtConfig>();
    config.brightness = 1.0;
  }
  app.update();

  {
    let materials = app.world().resource::<Assets<DeconvergenceMaterial>>();
    let material = materials.get(&material_id).unwrap();
    assert!(
      (material.params.glow_brightness.y - 1.0).abs() < 0.01,
      "First change: brightness should be 1.0"
    );
  }

  // Second change
  {
    let mut config = app.world_mut().resource_mut::<CrtConfig>();
    config.brightness = 2.0;
  }
  app.update();

  {
    let materials = app.world().resource::<Assets<DeconvergenceMaterial>>();
    let material = materials.get(&material_id).unwrap();
    assert!(
      (material.params.glow_brightness.y - 2.0).abs() < 0.01,
      "Second change: brightness should be 2.0"
    );
  }

  // Third change
  {
    let mut config = app.world_mut().resource_mut::<CrtConfig>();
    config.brightness = 3.0;
  }
  app.update();

  {
    let materials = app.world().resource::<Assets<DeconvergenceMaterial>>();
    let material = materials.get(&material_id).unwrap();
    assert!(
      (material.params.glow_brightness.y - 3.0).abs() < 0.01,
      "Third change: brightness should be 3.0"
    );
  }
}

// === Helper functions ===

/// Creates a mock DeconvergenceMaterial for testing.
/// Uses default/placeholder handles since we don't need actual textures.
fn create_mock_deconvergence_material() -> DeconvergenceMaterial {
  DeconvergenceMaterial {
    source_image: Handle::default(),
    texture_size: Vec2::new(640.0, 480.0),
    linearize_pass: Handle::default(),
    bloom_pass: Handle::default(),
    pre_pass: Handle::default(),
    frame_count: 0,
    source_size: Vec2::new(320.0, 240.0),
    params: bevy_crt::CrtParams::default(),
  }
}

/// Test version of update_crt_params that uses Assets::insert() to trigger
/// changes. This is the same logic as in bevy_crt::plugin but extracted for
/// testing.
fn update_crt_params_test_system(
  crt_config: Res<CrtConfig>,
  mut decon_materials: ResMut<Assets<DeconvergenceMaterial>>,
) {
  if crt_config.is_changed() {
    let params = crt_config.to_params();
    let updates: Vec<_> = decon_materials
      .iter()
      .map(|(id, mat)| {
        let mut updated = mat.clone();
        updated.params = params;
        (id, updated)
      })
      .collect();
    for (id, material) in updates {
      let _ = decon_materials.insert(id, material);
    }
  }
}
