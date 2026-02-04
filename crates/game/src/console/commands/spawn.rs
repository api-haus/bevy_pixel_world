//! Spawn command for pixel bodies.

use bevy::prelude::*;
use bevy_console::{ConsoleCommand, reply};
use clap::Parser;

use crate::pixel_world::pixel_body::SpawnPixelBody;
use crate::pixel_world::{Bomb, material_ids};
use crate::player::components::Player;

#[derive(Parser, ConsoleCommand)]
#[command(name = "spawn")]
pub struct SpawnCommand {
  /// Object type: bomb, femur, or box
  object: String,
}

pub fn spawn_command(
  mut log: ConsoleCommand<SpawnCommand>,
  players: Query<&Transform, With<Player>>,
  mut commands: Commands,
) {
  if let Some(Ok(SpawnCommand { object })) = log.take() {
    let Ok(player_transform) = players.single() else {
      reply!(log, "No player found");
      return;
    };

    let pos = Vec2::new(
      player_transform.translation.x,
      player_transform.translation.y + 50.0,
    );

    let sprite = match object.to_lowercase().as_str() {
      "bomb" => {
        commands.queue(
          SpawnPixelBody::new("sprites/cc0/box.png", material_ids::WOOD, pos).with_extra(
            |entity| {
              entity.insert(Bomb {
                damage_threshold: 0.03,
                blast_radius: 120.0,
                blast_strength: 60.0,
                detonated: false,
              });
            },
          ),
        );
        reply!(log, "Spawned bomb at ({:.0}, {:.0})", pos.x, pos.y);
        return;
      }
      "femur" => "sprites/cc0/femur.png",
      "box" => "sprites/cc0/box.png",
      _ => {
        reply!(log, "Unknown object: {}. Use: bomb, femur, or box", object);
        return;
      }
    };

    commands.queue(SpawnPixelBody::new(sprite, material_ids::WOOD, pos));
    reply!(log, "Spawned {} at ({:.0}, {:.0})", object, pos.x, pos.y);
  }
}
