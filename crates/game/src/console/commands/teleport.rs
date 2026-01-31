//! Teleport command.

use bevy::prelude::*;
use bevy_console::{ConsoleCommand, reply};
use clap::Parser;

use crate::core::camera::GameCamera;
#[cfg(feature = "editor")]
use crate::editor::GameMode;
use crate::player::components::Player;

#[derive(Parser, ConsoleCommand)]
#[command(name = "tp")]
pub struct TeleportCommand {
  /// X coordinate
  x: f32,
  /// Y coordinate
  y: f32,
}

pub fn teleport_command(
  mut log: ConsoleCommand<TeleportCommand>,
  mut players: Query<&mut Transform, With<Player>>,
  mut cameras: Query<&mut Transform, (With<GameCamera>, Without<Player>)>,
  #[cfg(feature = "editor")] game_mode: Res<State<GameMode>>,
) {
  if let Some(Ok(TeleportCommand { x, y })) = log.take() {
    #[cfg(feature = "editor")]
    let editing = *game_mode.get() == GameMode::Editing;
    #[cfg(not(feature = "editor"))]
    let editing = false;

    if editing {
      if let Ok(mut transform) = cameras.single_mut() {
        transform.translation.x = x;
        transform.translation.y = y;
        reply!(log, "Teleported camera to ({}, {})", x, y);
      } else {
        reply!(log, "No camera found");
      }
    } else if let Ok(mut transform) = players.single_mut() {
      transform.translation.x = x;
      transform.translation.y = y;
      reply!(log, "Teleported to ({}, {})", x, y);
    } else {
      reply!(log, "No player found");
    }
  }
}
