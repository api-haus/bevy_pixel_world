//! Developer console with commands for teleportation, time control, spawning,
//! and creative mode.

pub mod commands;
mod toggle;

use bevy::prelude::*;
use bevy_console::{AddConsoleCommand, ConsoleConfiguration, ConsolePlugin};
use commands::{
  CreativeCommand, SpawnCommand, TeleportCommand, TimeCommand, creative_command, spawn_command,
  teleport_command, time_command,
};

/// Current game mode.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
  #[default]
  Survival,
  Creative,
}

/// Position controlled by WASD in creative mode.
#[derive(Resource, Default)]
pub struct CreativeModePosition(pub Vec2);

/// Run condition that returns true when in survival mode.
pub fn in_survival_mode(mode: Res<GameMode>) -> bool {
  *mode == GameMode::Survival
}

/// Run condition that returns true when in creative mode.
pub fn in_creative_mode(mode: Res<GameMode>) -> bool {
  *mode == GameMode::Creative
}

pub struct ConsolePlugins;

impl Plugin for ConsolePlugins {
  fn build(&self, app: &mut App) {
    app
      .init_resource::<GameMode>()
      .init_resource::<CreativeModePosition>()
      .add_plugins(ConsolePlugin)
      .insert_resource(ConsoleConfiguration {
        // Disable default toggle keys, we use custom `/` handling
        keys: vec![],
        ..default()
      })
      .add_console_command::<TeleportCommand, _>(teleport_command)
      .add_console_command::<TimeCommand, _>(time_command)
      .add_console_command::<SpawnCommand, _>(spawn_command)
      .add_console_command::<CreativeCommand, _>(creative_command)
      .add_systems(Update, toggle::handle_console_toggle);
  }
}
