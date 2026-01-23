mod plugin;

use bevy::{asset::Asset, prelude::*, reflect::TypePath};
pub use plugin::ConfigPlugin;
use serde::Deserialize;

#[derive(Asset, TypePath, Deserialize, Debug, Clone)]
pub struct GameConfig {
  pub window: WindowConfig,
  pub camera: CameraConfig,
  pub physics: PhysicsConfig,
  pub player: PlayerConfig,
  pub ground: GroundConfig,
  pub platforms: PlatformsConfig,
}

#[derive(Deserialize, Debug, Clone)]
pub struct WindowConfig {
  pub width: u32,
  pub height: u32,
  pub title: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CameraConfig {
  pub clear_color: [f32; 3],
  pub viewport_width: f32,
  pub viewport_height: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PhysicsConfig {
  pub gravity: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PlayerConfig {
  pub color: [f32; 3],
  pub width: f32,
  pub height: f32,
  pub spawn_x: f32,
  pub spawn_y: f32,
  pub collider_radius: f32,
  pub collider_length: f32,
  pub sensor_width: f32,
  pub sensor_height: f32,
  pub float_height: f32,
  pub walk_speed: f32,
  pub acceleration: f32,
  pub air_acceleration: f32,
  pub flight_speed: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct GroundConfig {
  pub color: [f32; 3],
  pub width: f32,
  pub height: f32,
  pub y_position: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PlatformsConfig {
  pub seed: u64,
  pub count: u32,
  pub width_min: f32,
  pub width_max: f32,
  pub height: f32,
  pub x_min: f32,
  pub x_max: f32,
  pub y_min: f32,
  pub y_max: f32,
  pub color: [f32; 3],
}

#[derive(Resource)]
pub struct ConfigHandle(pub Handle<GameConfig>);

#[derive(Resource, Debug, Clone)]
pub struct ConfigLoaded {
  pub window: WindowConfig,
  pub camera: CameraConfig,
  pub physics: PhysicsConfig,
  pub player: PlayerConfig,
  pub ground: GroundConfig,
  pub platforms: PlatformsConfig,
}

impl From<GameConfig> for ConfigLoaded {
  fn from(config: GameConfig) -> Self {
    Self {
      window: config.window,
      camera: config.camera,
      physics: config.physics,
      player: config.player,
      ground: config.ground,
      platforms: config.platforms,
    }
  }
}
