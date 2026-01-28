#[cfg(not(target_family = "wasm"))]
use bevy::{asset::AssetEvent, ecs::message::MessageReader};
use bevy::{camera::ScalingMode, prelude::*, window::PrimaryWindow};
#[cfg(not(target_family = "wasm"))]
use bevy_common_assets::toml::TomlAssetPlugin;

#[cfg(not(target_family = "wasm"))]
use super::ConfigHandle;
use super::{ConfigLoaded, GameConfig};
use crate::core::GravityConfig;

pub struct ConfigPlugin;

impl Plugin for ConfigPlugin {
  fn build(&self, app: &mut App) {
    // Native: asset-based config with hot-reload
    #[cfg(not(target_family = "wasm"))]
    app
      .add_plugins(TomlAssetPlugin::<GameConfig>::new(&["config.toml"]))
      .add_systems(Update, watch_config_changes);

    app.add_systems(PreStartup, load_config_sync).add_systems(
      Update,
      (
        update_window_on_config_change,
        update_gravity_on_config_change,
        update_camera_on_config_change,
      ),
    );
  }
}

fn load_config_sync(
  mut commands: Commands,
  #[cfg(not(target_family = "wasm"))] asset_server: Res<AssetServer>,
) {
  // Native: set up asset handle for hot-reload
  #[cfg(not(target_family = "wasm"))]
  {
    let handle: Handle<GameConfig> = asset_server.load("config/game.config.toml");
    commands.insert_resource(ConfigHandle(handle));
  }

  // WASM: embed config at compile time
  #[cfg(target_family = "wasm")]
  let config_str = include_str!("../../assets/config/game.config.toml");
  #[cfg(not(target_family = "wasm"))]
  let config_str =
    std::fs::read_to_string("assets/config/game.config.toml").expect("Failed to read config file");

  let config: GameConfig = toml::from_str(&config_str).expect("Failed to parse config file");

  commands.insert_resource(ConfigLoaded::from(config));
}

#[cfg(not(target_family = "wasm"))]
fn watch_config_changes(
  mut commands: Commands,
  config_handle: Res<ConfigHandle>,
  mut messages: MessageReader<AssetEvent<GameConfig>>,
  configs: Res<Assets<GameConfig>>,
) {
  for event in messages.read() {
    if let AssetEvent::Modified { id } = event {
      if config_handle.0.id() == *id {
        if let Some(config) = configs.get(&config_handle.0) {
          info!("Config reloaded!");
          commands.insert_resource(ConfigLoaded::from(config.clone()));
        }
      }
    }
  }
}

fn update_window_on_config_change(
  config: Res<ConfigLoaded>,
  mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
  if config.is_changed() {
    if let Ok(mut window) = windows.single_mut() {
      window
        .resolution
        .set(config.window.width as f32, config.window.height as f32);
      window.title.clone_from(&config.window.title);
    }
  }
}

fn update_gravity_on_config_change(config: Res<ConfigLoaded>, mut gravity: ResMut<GravityConfig>) {
  if config.is_changed() {
    gravity.value = config.physics.gravity;
  }
}

fn update_camera_on_config_change(
  config: Res<ConfigLoaded>,
  mut camera_query: Query<&mut Projection, With<Camera2d>>,
) {
  if config.is_changed() {
    for mut projection in camera_query.iter_mut() {
      if let Projection::Orthographic(ref mut ortho) = *projection {
        ortho.scaling_mode = ScalingMode::AutoMin {
          min_width: config.camera.viewport_width,
          min_height: config.camera.viewport_height,
        };
      }
    }
  }
}
