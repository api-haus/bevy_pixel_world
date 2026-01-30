//! Teleport command.

use bevy::prelude::*;
use bevy_console::{ConsoleCommand, reply};
use clap::Parser;

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
) {
  if let Some(Ok(TeleportCommand { x, y })) = log.take() {
    if let Ok(mut transform) = players.single_mut() {
      transform.translation.x = x;
      transform.translation.y = y;
      reply!(log, "Teleported to ({}, {})", x, y);
    } else {
      reply!(log, "No player found");
    }
  }
}
