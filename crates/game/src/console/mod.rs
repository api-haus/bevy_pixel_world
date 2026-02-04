//! Developer console with commands for teleportation, time control, spawning,
//! and creative mode.

pub mod commands;
mod history;
mod toggle;

use bevy::prelude::*;
use bevy_console::{AddConsoleCommand, ConsoleConfiguration, ConsolePlugin, ConsoleSet};
use bevy_egui::EguiPrimaryContextPass;
use commands::{
  CreativeCommand, SaveCommand, SpawnCommand, TeleportCommand, TimeCommand, VisualCommand,
  creative_command, notify_save_complete, save_command, spawn_command, teleport_command,
  time_command, visual_command,
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

pub struct ConsolePlugins;

impl Plugin for ConsolePlugins {
  fn build(&self, app: &mut App) {
    app
      .init_resource::<GameMode>()
      .init_resource::<CreativeModePosition>()
      .add_plugins(ConsolePlugin)
      .insert_resource(ConsoleConfiguration {
        // Use F12 as hidden toggle key for synthetic events from our `/` handler
        keys: vec![KeyCode::F12],
        height: 300.0,
        ..default()
      })
      .add_console_command::<TeleportCommand, _>(teleport_command)
      .add_console_command::<TimeCommand, _>(time_command)
      .add_console_command::<SpawnCommand, _>(spawn_command)
      .add_console_command::<CreativeCommand, _>(creative_command)
      .add_console_command::<SaveCommand, _>(save_command)
      .add_console_command::<VisualCommand, _>(visual_command)
      .add_systems(Update, notify_save_complete)
      .add_systems(Update, toggle::handle_console_toggle)
      .add_systems(Update, toggle::handle_backspace_workaround)
      .add_systems(EguiPrimaryContextPass, toggle::maintain_console_focus)
      // History persistence: load after ConsolePlugin startup, then track and save
      .add_systems(
        Startup,
        (history::load_history, history::init_persistence)
          .chain()
          .after(ConsoleSet::Startup),
      )
      .add_systems(
        Update,
        (history::track_history_changes, history::save_history).chain(),
      );
  }
}
