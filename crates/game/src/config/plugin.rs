use bevy::{
  asset::AssetEvent, camera::ScalingMode, ecs::message::MessageReader, prelude::*,
  window::PrimaryWindow,
};
use bevy_common_assets::toml::TomlAssetPlugin;

use super::{ConfigLoaded, GameConfig};
use crate::core::camera::GameCamera;
use crate::core::GravityConfig;
use crate::platform::{EmbeddedAssets, PlatformConfig};

pub struct ConfigPlugin;

impl Plugin for ConfigPlugin {
  fn build(&self, app: &mut App) {
    // Check platform config to determine if hot-reload should be enabled
    let hot_reload = app.world().resource::<PlatformConfig>().hot_reload;

    if hot_reload {
      app
        .add_plugins(TomlAssetPlugin::<GameConfig>::new(&["config.toml"]))
        .add_systems(Update, watch_config_changes);
    }

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
  embedded: Option<Res<EmbeddedAssets>>,
  platform: Res<PlatformConfig>,
  asset_server: Res<AssetServer>,
) {
  // Set up asset handle for hot-reload if enabled
  if platform.hot_reload {
    let handle: Handle<GameConfig> = asset_server.load("config/game.config.toml");
    commands.insert_resource(super::ConfigHandle(handle));
  }

  // Load config from embedded assets or filesystem
  let config_str = embedded
    .as_ref()
    .map(|e| e.game_config.to_string())
    .unwrap_or_else(|| {
      std::fs::read_to_string("assets/config/game.config.toml").expect("Failed to read config file")
    });

  let config: GameConfig = toml::from_str(&config_str).expect("Failed to parse config file");
  commands.insert_resource(ConfigLoaded::from(config));
}

fn watch_config_changes(
  mut commands: Commands,
  config_handle: Option<Res<super::ConfigHandle>>,
  mut messages: MessageReader<AssetEvent<GameConfig>>,
  configs: Res<Assets<GameConfig>>,
) {
  let Some(config_handle) = config_handle else {
    return;
  };

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
  mut camera_query: Query<&mut Projection, With<GameCamera>>,
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
