mod config;
mod core;
#[cfg(feature = "editor")]
mod editor;
mod input;
mod player;
mod visual_debug;
mod world;

use bevy::{
  prelude::*,
  window::{MonitorSelection, PresentMode, WindowMode, WindowResolution},
};

fn main() {
  // Read config synchronously for initial window setup
  let config_path = std::path::Path::new("assets/config/game.config.toml");
  let config_str = std::fs::read_to_string(config_path).expect("Failed to read config file");
  let config: config::GameConfig = toml::from_str(&config_str).expect("Failed to parse config");

  let mut app = App::new();

  app.insert_resource(Time::<Fixed>::from_hz(60.0));

  app
    .add_plugins(
      DefaultPlugins
        .set(ImagePlugin::default_nearest())
        .set(WindowPlugin {
          primary_window: Some(Window {
            resolution: WindowResolution::new(config.window.width, config.window.height),
            title: config.window.title.clone(),
            present_mode: PresentMode::Immediate, // No buffering - test for ghost
            mode: WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
            ..default()
          }),
          ..default()
        }),
    )
    .add_plugins(config::ConfigPlugin)
    .add_plugins(core::CorePlugin)
    .add_plugins(input::InputPlugin)
    .add_plugins(player::PlayerPlugin);

  // Editor mode: use yoleck level files
  #[cfg(feature = "editor")]
  app.add_plugins(editor::EditorPlugin);

  // Non-editor mode: use procedural world generation
  #[cfg(not(feature = "editor"))]
  app.add_plugins(world::WorldPlugin);

  app.add_plugins(visual_debug::VisualDebugPlugin);
  app.add_plugins(bevy_pixel_world::diagnostics::DiagnosticsPlugin);

  app.run();
}
