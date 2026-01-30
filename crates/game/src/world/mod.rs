//! World setup with PixelWorld integration.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_pixel_world::{
  BrushUiPlugin, MaterialSeeder, Materials, MaterialsConfig, PersistenceConfig,
  PixelWorldFullBundle, SpawnPixelWorld,
};

use crate::platform::{EmbeddedAssets, PlatformConfig};

/// Configuration for the world plugin.
pub struct WorldPlugin {
  /// Path to materials.toml config file (used when EmbeddedAssets is not
  /// available).
  pub materials_config_path: PathBuf,
}

impl WorldPlugin {
  /// Create a new WorldPlugin with the specified materials config path.
  pub fn new(materials_config_path: impl Into<PathBuf>) -> Self {
    Self {
      materials_config_path: materials_config_path.into(),
    }
  }
}

impl Plugin for WorldPlugin {
  fn build(&self, app: &mut App) {
    // Load materials config from embedded assets or filesystem
    let embedded = app.world().get_resource::<EmbeddedAssets>();
    let config_str = embedded
      .map(|e| e.materials_config.to_string())
      .unwrap_or_else(|| {
        std::fs::read_to_string(&self.materials_config_path).unwrap_or_else(|e| {
          panic!(
            "Failed to read materials config from {:?}: {}",
            self.materials_config_path, e
          )
        })
      });
    let config: MaterialsConfig =
      toml::from_str(&config_str).expect("Failed to parse materials config");

    // Get save path from platform config
    let platform = app.world().resource::<PlatformConfig>();
    let save_path = platform.save_dir.join("world.save");

    app
      .insert_resource(Materials::from(config))
      .add_plugins(PixelWorldFullBundle::new(PersistenceConfig::at(save_path)))
      .add_plugins(bevy_pixel_world::PixelDebugControllerPlugin)
      .add_plugins(BrushUiPlugin)
      .add_plugins(bevy_pixel_world::BasicPersistencePlugin)
      .add_systems(Startup, spawn_world);
  }
}

fn spawn_world(mut commands: Commands) {
  commands.queue(SpawnPixelWorld::new(MaterialSeeder::new(42)));
}
