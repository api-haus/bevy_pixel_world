//! Creative mode toggle command.

use bevy::prelude::*;
use bevy_console::{ConsoleCommand, reply};
use bevy_pixel_world::BrushUiVisible;
use clap::Parser;

use crate::console::{CreativeModePosition, GameMode};
use crate::player::components::Player;

#[derive(Parser, ConsoleCommand)]
#[command(name = "creative")]
pub struct CreativeCommand;

pub fn creative_command(
  mut log: ConsoleCommand<CreativeCommand>,
  mut mode: ResMut<GameMode>,
  mut brush_ui: Option<ResMut<BrushUiVisible>>,
  mut creative_pos: ResMut<CreativeModePosition>,
  players: Query<&Transform, With<Player>>,
) {
  if let Some(Ok(CreativeCommand)) = log.take() {
    *mode = match *mode {
      GameMode::Survival => GameMode::Creative,
      GameMode::Creative => GameMode::Survival,
    };
    let is_creative = *mode == GameMode::Creative;
    if let Some(ref mut brush_ui) = brush_ui {
      brush_ui.0 = is_creative;
    }
    if is_creative {
      // Initialize creative position from current player position
      if let Ok(transform) = players.single() {
        creative_pos.0 = transform.translation.truncate();
      }
      reply!(log, "Creative mode enabled");
    } else {
      reply!(log, "Creative mode disabled");
    }
  }
}
