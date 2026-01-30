mod plugin;

use bevy::{asset::Asset, prelude::*, reflect::TypePath};
pub use plugin::ConfigPlugin;
use serde::{Deserialize, Deserializer, de};

#[derive(Asset, TypePath, Deserialize, Debug, Clone)]
pub struct GameConfig {
  pub window: WindowConfig,
  pub camera: CameraConfig,
  pub physics: PhysicsConfig,
  pub player: PlayerConfig,
  pub ground: GroundConfig,
  pub platforms: PlatformsConfig,
  pub day_cycle: DayCycleConfig,
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
  pub snap_to_ground: f32,
  pub max_slope_angle: f32,
  pub autostep_height: f32,
  pub autostep_width: f32,
  pub sprite: String,
  pub sprite_scale: f32,
  pub sprite_pivot: [f32; 2],
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

#[derive(Deserialize, Debug, Clone)]
pub struct DayCycleConfig {
  pub seconds_per_hour: f32,
  pub start_hour: f32,
  pub sky_colors: Vec<SkyKeyframe>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SkyKeyframe {
  pub hour: f32,
  #[serde(deserialize_with = "deserialize_hex_color")]
  pub color: [f32; 3],
}

fn deserialize_hex_color<'de, D>(deserializer: D) -> Result<[f32; 3], D::Error>
where
  D: Deserializer<'de>,
{
  let s: String = Deserialize::deserialize(deserializer)?;
  let s = s.trim_start_matches('#');
  if s.len() != 6 {
    return Err(de::Error::custom("hex color must be 6 characters"));
  }
  let r = u8::from_str_radix(&s[0..2], 16).map_err(de::Error::custom)?;
  let g = u8::from_str_radix(&s[2..4], 16).map_err(de::Error::custom)?;
  let b = u8::from_str_radix(&s[4..6], 16).map_err(de::Error::custom)?;
  Ok([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0])
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
  pub day_cycle: DayCycleConfig,
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
      day_cycle: config.day_cycle,
    }
  }
}
