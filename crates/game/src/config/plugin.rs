use bevy::{asset::AssetEvent, ecs::message::MessageReader, prelude::*, window::PrimaryWindow};
use bevy_common_assets::toml::TomlAssetPlugin;

use super::{ConfigHandle, ConfigLoaded, GameConfig};
use crate::core::GravityConfig;

pub struct ConfigPlugin;

impl Plugin for ConfigPlugin {
  fn build(&self, app: &mut App) {
    app
      .add_plugins(TomlAssetPlugin::<GameConfig>::new(&["config.toml"]))
      .add_systems(PreStartup, load_config_sync)
      .add_systems(
        Update,
        (
          watch_config_changes,
          update_window_on_config_change,
          update_gravity_on_config_change,
        ),
      );
  }
}

fn load_config_sync(mut commands: Commands, asset_server: Res<AssetServer>) {
  let handle: Handle<GameConfig> = asset_server.load("config/game.config.toml");
  commands.insert_resource(ConfigHandle(handle));

  let config_path = std::path::Path::new("assets/config/game.config.toml");
  let config_str = std::fs::read_to_string(config_path).expect("Failed to read config file");
  let config: GameConfig = toml::from_str(&config_str).expect("Failed to parse config file");

  commands.insert_resource(ConfigLoaded::from(config));
}

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
