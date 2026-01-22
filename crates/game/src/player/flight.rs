use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

use super::components::{CharacterMovementConfig, CharacterVelocity, LocomotionState, Player};
use crate::input::{Fly, PlayerInput};

/// Handles input-driven state transitions.
/// Critical: Zeros vertical velocity on Flying → Airborne transition to prevent
/// pass-through.
pub fn process_locomotion_transitions(
  mut players: Query<
    (
      &mut LocomotionState,
      &mut CharacterVelocity,
      &CharacterMovementConfig,
      &Actions<PlayerInput>,
    ),
    With<Player>,
  >,
  action_states: Query<&ActionState, With<Action<Fly>>>,
) {
  for (mut state, mut velocity, config, actions) in &mut players {
    let mut fly_pressed = false;

    for action_entity in actions.iter() {
      if let Ok(action_state) = action_states.get(action_entity) {
        // Fired = just pressed, Ongoing = held
        if matches!(action_state, ActionState::Fired | ActionState::Ongoing) {
          fly_pressed = true;
        }
      }
    }

    match *state {
      LocomotionState::Grounded | LocomotionState::Airborne => {
        if fly_pressed {
          *state = LocomotionState::Flying;
          velocity.0.y = config.flight_speed;
        }
      }
      LocomotionState::Flying => {
        if fly_pressed {
          // Continue flying - set velocity
          velocity.0.y = config.flight_speed;
        } else {
          // Stopped flying - zero velocity and transition to airborne
          velocity.0.y = 0.0;
          *state = LocomotionState::Airborne;
        }
      }
    }
  }
}
