//! Player ability to spawn pixel bodies.

use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;
use rand::Rng;

use super::components::Player;
use crate::input::actions::{PlayerInput, SpawnBody};
use crate::pixel_world::pixel_body::SpawnPixelBody;
use crate::pixel_world::{Bomb, PixelBody, material_ids};

/// Tracks whether we've spawned a body this press to avoid repeat spawns.
#[derive(Resource, Default)]
pub struct SpawnBodyState {
  spawned_this_press: bool,
}

/// System that spawns a pixel body when the player presses F.
pub fn spawn_body_on_input(
  players: Query<(&Transform, &Actions<PlayerInput>), With<Player>>,
  action_states: Query<&ActionState, With<Action<SpawnBody>>>,
  mut commands: Commands,
  mut state: Local<SpawnBodyState>,
) {
  for (player_transform, actions) in &players {
    for action_entity in actions.iter() {
      let Ok(action_state) = action_states.get(action_entity) else {
        continue;
      };

      match action_state {
        ActionState::Fired => {
          // Only spawn once per press
          if !state.spawned_this_press {
            state.spawned_this_press = true;

            let pos = Vec2::new(
              player_transform.translation.x,
              player_transform.translation.y + 50.0, // Spawn above player
            );

            // Random sprite selection
            let mut rng = rand::thread_rng();
            let sprite = if rng.gen_bool(0.5) {
              "box.png"
            } else {
              "femur.png"
            };

            commands.queue(SpawnPixelBody::new(sprite, material_ids::WOOD, pos));
          }
        }
        ActionState::None => {
          // Reset when key is released
          state.spawned_this_press = false;
        }
        _ => {}
      }
    }
  }
}

/// Tags newly spawned pixel bodies as bombs.
pub fn tag_new_bodies_as_bombs(
  mut commands: Commands,
  new_bodies: Query<Entity, Added<PixelBody>>,
) {
  for entity in &new_bodies {
    commands.entity(entity).insert(Bomb {
      damage_threshold: 0.03,
      blast_radius: 120.0,
      blast_strength: 60.0,
      detonated: false,
    });
  }
}
