//! Platform-specific initialization and configuration.
//!
//! This module centralizes all platform differences so game logic can remain
//! platform-agnostic. The `init()` function returns platform-appropriate
//! configuration that other modules consume via resources.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowMode};

/// Platform-specific configuration used by window setup and other systems.
#[derive(Resource, Debug, Clone)]
pub struct PlatformConfig {
  /// Window mode: Windowed on WASM, BorderlessFullscreen on native.
  pub window_mode: WindowMode,
  /// Present mode: Fifo (vsync) on WASM, Immediate on native.
  pub present_mode: PresentMode,
  /// Canvas selector for WASM, None on native.
  pub canvas: Option<String>,
  /// Whether to fit canvas to parent element (WASM only).
  pub fit_canvas_to_parent: bool,
  /// Whether to prevent default browser event handling (context menu, etc).
  pub prevent_default_event_handling: bool,
  /// Directory for save files.
  pub save_dir: PathBuf,
  /// Whether hot-reload is enabled (native only).
  pub hot_reload: bool,
}

/// Embedded asset strings for WASM builds where filesystem access is
/// unavailable.
#[derive(Resource, Debug, Clone)]
pub struct EmbeddedAssets {
  /// Contents of game.config.toml
  pub game_config: &'static str,
  /// Contents of materials.toml
  pub materials_config: &'static str,
}

/// Initialize platform-specific configuration.
///
/// Returns `PlatformConfig` for all platforms, and `EmbeddedAssets` only on
/// WASM.
pub fn init() -> (PlatformConfig, Option<EmbeddedAssets>) {
  #[cfg(target_family = "wasm")]
  console_error_panic_hook::set_once();

  let config = PlatformConfig {
    #[cfg(target_family = "wasm")]
    window_mode: WindowMode::Windowed,
    #[cfg(not(target_family = "wasm"))]
    window_mode: WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Primary),

    #[cfg(target_family = "wasm")]
    present_mode: PresentMode::Fifo,
    #[cfg(not(target_family = "wasm"))]
    present_mode: PresentMode::Immediate,

    #[cfg(target_family = "wasm")]
    canvas: Some("#bevy".to_string()),
    #[cfg(not(target_family = "wasm"))]
    canvas: None,

    #[cfg(target_family = "wasm")]
    fit_canvas_to_parent: true,
    #[cfg(not(target_family = "wasm"))]
    fit_canvas_to_parent: false,

    // Prevent browser context menu on right-click (WASM)
    #[cfg(target_family = "wasm")]
    prevent_default_event_handling: true,
    #[cfg(not(target_family = "wasm"))]
    prevent_default_event_handling: false,

    #[cfg(target_family = "wasm")]
    save_dir: PathBuf::from("."),
    #[cfg(not(target_family = "wasm"))]
    save_dir: dirs::data_dir()
      .unwrap_or_else(|| PathBuf::from("."))
      .join("sim2d_game")
      .join("saves"),

    #[cfg(target_family = "wasm")]
    hot_reload: false,
    #[cfg(not(target_family = "wasm"))]
    hot_reload: true,
  };

  #[cfg(target_family = "wasm")]
  let embedded = Some(EmbeddedAssets {
    game_config: include_str!("../assets/config/game.config.toml"),
    materials_config: include_str!("../assets/config/materials.toml"),
  });
  #[cfg(not(target_family = "wasm"))]
  let embedded = None;

  (config, embedded)
}
