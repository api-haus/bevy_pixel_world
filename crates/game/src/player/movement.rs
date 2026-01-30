use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;
use bevy_rapier2d::prelude::*;

use super::components::{CharacterMovementConfig, CharacterVelocity, LocomotionState, Player};
use crate::core::GravityConfig;
use crate::input::{Move, PlayerInput};

/// Runs in FixedPostUpdate AFTER physics to read fresh ground state.
pub fn sync_ground_from_physics(
  mut players: Query<
    (
      &mut LocomotionState,
      &mut CharacterVelocity,
      Option<&KinematicCharacterControllerOutput>,
    ),
    With<Player>,
  >,
) {
  for (mut state, mut velocity, output) in &mut players {
    let physics_grounded = output.is_some_and(|o| o.grounded);

    match *state {
      LocomotionState::Grounded => {
        if !physics_grounded {
          *state = LocomotionState::Airborne;
        }
      }
      LocomotionState::Airborne => {
        if physics_grounded {
          // Landing! Zero vertical velocity and transition to grounded
          velocity.0.y = 0.0;
          *state = LocomotionState::Grounded;
        }
      }
      LocomotionState::Flying => {
        // Flying state is only changed by input, not physics
      }
    }
  }
}

pub fn handle_movement_input(
  mut players: Query<
    (
      &Actions<PlayerInput>,
      &mut CharacterVelocity,
      &CharacterMovementConfig,
      &LocomotionState,
    ),
    With<Player>,
  >,
  move_actions: Query<(&Action<Move>, &ActionState)>,
  time: Res<Time>,
) {
  for (actions, mut velocity, config, state) in &mut players {
    let mut move_value = 0.0;
    for action_entity in actions.iter() {
      if let Ok((action, action_state)) = move_actions.get(action_entity) {
        let raw_value = **action;
        // Only use input when action is active (Fired or Ongoing)
        if matches!(action_state, ActionState::Fired | ActionState::Ongoing) {
          move_value = raw_value;
        }
        // Log when there's movement
        if raw_value != 0.0 || velocity.0.x.abs() > 1.0 {
          trace!(
            "Move: raw={}, state={:?}, vel_x={:.1}, loco={:?}",
            raw_value, action_state, velocity.0.x, state
          );
        }
      }
    }

    let target_velocity_x = move_value * config.walk_speed;
    let accel = if *state == LocomotionState::Grounded {
      config.acceleration
    } else {
      config.air_acceleration
    };

    // Smoothly interpolate horizontal velocity towards target
    let diff = target_velocity_x - velocity.0.x;
    velocity.0.x += diff * accel * time.delta_secs();
  }
}

/// Applies gravity based on locomotion state. Flight velocity is set in
/// process_locomotion_transitions.
pub fn apply_locomotion_physics(
  mut players: Query<(&mut CharacterVelocity, &LocomotionState), With<Player>>,
  gravity: Res<GravityConfig>,
  time: Res<Time>,
) {
  const TERMINAL_VELOCITY: f32 = 500.0;

  for (mut velocity, state) in &mut players {
    match state {
      LocomotionState::Grounded => {
        // Keep velocity.y at 0 when grounded
        velocity.0.y = 0.0;
      }
      LocomotionState::Airborne => {
        // Apply gravity, clamp to terminal velocity
        velocity.0.y -= gravity.value * time.delta_secs();
        velocity.0.y = velocity.0.y.max(-TERMINAL_VELOCITY);
      }
      LocomotionState::Flying => {
        // Flight velocity is set by process_locomotion_transitions
        // Don't modify here to avoid overwriting the zeroing on exit
      }
    }
  }
}

pub fn apply_velocity_to_controller(
  mut players: Query<(&CharacterVelocity, &mut KinematicCharacterController), With<Player>>,
  time: Res<Time>,
) {
  for (velocity, mut controller) in &mut players {
    controller.translation = Some(velocity.0 * time.delta_secs());
  }
}
