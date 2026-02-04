mod ambiance;
mod config;
mod console;
mod core;
#[cfg(feature = "editor")]
mod editor;
mod input;
mod platform;
mod player;
mod time_of_day;
mod visual_debug;
mod world;

use bevy::{asset::AssetMetaCheck, prelude::*, window::WindowResolution};
use game::pixel_world;

fn main() {
  let (platform_config, embedded_assets) = platform::init();

  // Load game config for window setup
  let config_str = embedded_assets
    .as_ref()
    .map(|e| e.game_config.to_string())
    .unwrap_or_else(|| {
      std::fs::read_to_string("assets/config/game.config.toml").expect("Failed to read config file")
    });
  let config: config::GameConfig = toml::from_str(&config_str).expect("Failed to parse config");

  let mut app = App::new();

  app.insert_resource(Time::<Fixed>::from_hz(60.0));
  app.insert_resource(platform_config.clone());
  if let Some(embedded) = embedded_assets {
    app.insert_resource(embedded);
  }

  app
    .add_plugins(
      DefaultPlugins
        .set(bevy::log::LogPlugin {
          filter: if cfg!(target_family = "wasm") {
            "error".to_string()
          } else {
            "info".to_string()
          },
          ..default()
        })
        .set(bevy::asset::AssetPlugin {
          // On web, servers may return HTML 404 pages instead of proper 404 status codes
          // when .meta files don't exist. Bevy then tries to parse HTML as RON and fails.
          // Since this project doesn't use .meta files, skip checking for them entirely.
          meta_check: AssetMetaCheck::Never,
          ..default()
        })
        .set(ImagePlugin::default_nearest())
        .set(WindowPlugin {
          primary_window: Some(Window {
            resolution: WindowResolution::new(config.window.width, config.window.height),
            title: config.window.title.clone(),
            present_mode: platform_config.present_mode,
            mode: platform_config.window_mode,
            canvas: platform_config.canvas.clone(),
            fit_canvas_to_parent: platform_config.fit_canvas_to_parent,
            prevent_default_event_handling: platform_config.prevent_default_event_handling,
            ..default()
          }),
          ..default()
        })
        // Disable 3D PBR plugin - removes SSAO, atmosphere, environment map warnings on WebGL2
        .disable::<bevy::pbr::PbrPlugin>(),
    )
    .add_plugins(config::ConfigPlugin)
    .add_plugins(core::CorePlugin)
    .add_plugins(time_of_day::TimeOfDayPlugin)
    .add_plugins(ambiance::Ambiance2DPlugin)
    .add_plugins(input::InputPlugin)
    .add_plugins(player::PlayerPlugin);

  // Always add procedural world with noise terrain
  app.add_plugins(world::WorldPlugin::new("assets/config/materials.toml"));

  // Editor mode: add level editor on top
  #[cfg(feature = "editor")]
  app.add_plugins(editor::EditorPlugin);

  app.add_plugins(visual_debug::VisualDebugPlugin);
  app.add_plugins(console::ConsolePlugins);
  // DiagnosticsPlugin is already added by PixelWorldFullBundle

  app.run();
}
