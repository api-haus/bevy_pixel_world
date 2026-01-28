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
  // WASM: set up panic hook for better error messages
  #[cfg(target_family = "wasm")]
  console_error_panic_hook::set_once();

  // WASM: embed config at compile time (no filesystem access)
  #[cfg(target_family = "wasm")]
  let config_str = include_str!("../assets/config/game.config.toml");
  #[cfg(not(target_family = "wasm"))]
  let config_str =
    std::fs::read_to_string("assets/config/game.config.toml").expect("Failed to read config file");

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
            // WASM: only Fifo (vsync) is supported on WebGL2
            #[cfg(target_family = "wasm")]
            present_mode: PresentMode::Fifo,
            #[cfg(not(target_family = "wasm"))]
            present_mode: PresentMode::Immediate,
            // WASM: use windowed mode and target canvas element
            #[cfg(target_family = "wasm")]
            mode: WindowMode::Windowed,
            #[cfg(target_family = "wasm")]
            canvas: Some("#bevy".to_string()),
            #[cfg(target_family = "wasm")]
            fit_canvas_to_parent: true,
            // Native: borderless fullscreen
            #[cfg(not(target_family = "wasm"))]
            mode: WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
            ..default()
          }),
          ..default()
        })
        // Disable 3D PBR plugin - removes SSAO, atmosphere, environment map warnings on WebGL2
        .disable::<bevy::pbr::PbrPlugin>(),
    )
    .add_plugins(config::ConfigPlugin)
    .add_plugins(core::CorePlugin)
    .add_plugins(input::InputPlugin)
    .add_plugins(player::PlayerPlugin);

  // Always add procedural world with noise terrain
  app.add_plugins(world::WorldPlugin);

  // Editor mode: add level editor on top
  #[cfg(feature = "editor")]
  app.add_plugins(editor::EditorPlugin);

  app.add_plugins(visual_debug::VisualDebugPlugin);
  // DiagnosticsPlugin is already added by PixelWorldFullBundle

  app.run();
}
