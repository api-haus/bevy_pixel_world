//! World setup with PixelWorld integration.

use bevy::prelude::*;
use bevy_pixel_world::{
  MaterialSeeder, Materials, MaterialsConfig, PersistenceConfig, PixelWorldFullBundle,
  SpawnPixelWorld,
};

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
  fn build(&self, app: &mut App) {
    // Load materials config
    #[cfg(target_family = "wasm")]
    let config: MaterialsConfig =
      toml::from_str(include_str!("../../assets/materials.toml")).unwrap();
    #[cfg(not(target_family = "wasm"))]
    let config: MaterialsConfig =
      toml::from_str(&std::fs::read_to_string("assets/materials.toml").unwrap()).unwrap();

    // Compute save path
    #[cfg(not(target_family = "wasm"))]
    let save_path = dirs::data_dir()
      .unwrap_or_else(|| std::path::PathBuf::from("."))
      .join("sim2d_game")
      .join("saves")
      .join("world.save");
    #[cfg(target_family = "wasm")]
    let save_path = std::path::PathBuf::from("world.save");

    app
      .insert_resource(Materials::from(config))
      .add_plugins(PixelWorldFullBundle::default().persistence(PersistenceConfig::at(save_path)))
      .add_plugins(bevy_pixel_world::PixelDebugControllerPlugin)
      .add_plugins(bevy_pixel_world::BasicPersistencePlugin)
      .add_systems(Startup, spawn_world);
  }
}

fn spawn_world(mut commands: Commands) {
  commands.queue(SpawnPixelWorld::new(MaterialSeeder::new(42)));
}
