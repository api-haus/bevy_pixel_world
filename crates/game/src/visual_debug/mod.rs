use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

use crate::input::{Move, PlayerInput};
use crate::player::components::{CharacterVelocity, Player};

/// Resource for frame-by-frame debug mode
#[derive(Resource, Default)]
pub struct FrameStepMode {
  pub enabled: bool,
  advance_requested: bool,
}

pub struct VisualDebugPlugin;

impl Plugin for VisualDebugPlugin {
  fn build(&self, app: &mut App) {
    app
      .init_resource::<FrameStepMode>()
      .add_systems(PreUpdate, frame_step_control)
      .add_systems(Update, draw_debug_vectors);
  }
}

/// Controls frame-by-frame stepping mode
/// F5: Toggle frame-step mode
/// Right Arrow: Advance one frame (when in frame-step mode)
fn frame_step_control(
  keyboard: Res<ButtonInput<KeyCode>>,
  mut frame_step: ResMut<FrameStepMode>,
  mut time: ResMut<Time<Virtual>>,
) {
  // Toggle frame-step mode with F5
  if keyboard.just_pressed(KeyCode::F5) {
    frame_step.enabled = !frame_step.enabled;
    if frame_step.enabled {
      time.pause();
      info!("Frame-step mode ENABLED (press Right Arrow to advance, F5 to disable)");
    } else {
      time.unpause();
      info!("Frame-step mode DISABLED");
    }
  }

  // Handle frame advancing
  if frame_step.enabled {
    if keyboard.just_pressed(KeyCode::ArrowRight) {
      // Request advance - unpause for this frame
      frame_step.advance_requested = true;
      time.unpause();
    } else if frame_step.advance_requested {
      // Previous frame was an advance, pause again
      frame_step.advance_requested = false;
      time.pause();
    }
  }
}

/// Visual debug system - draws velocity (yellow) and input (green) vectors
fn draw_debug_vectors(
  mut gizmos: Gizmos,
  players: Query<
    (&Transform, &CharacterVelocity, &Actions<PlayerInput>),
    With<Player>,
  >,
  move_actions: Query<(&Action<Move>, &ActionState)>,
) {
  const VELOCITY_SCALE: f32 = 0.5; // Scale factor for velocity visualization
  const INPUT_LENGTH: f32 = 50.0; // Fixed length for input vector

  for (transform, velocity, actions) in &players {
    let player_pos = transform.translation.truncate();

    // Draw velocity vector (yellow)
    if velocity.0.length_squared() > 0.01 {
      let velocity_end = player_pos + velocity.0 * VELOCITY_SCALE;
      gizmos.line_2d(player_pos, velocity_end, Color::srgb(1.0, 1.0, 0.0));
    }

    // Draw input vector (green)
    let mut move_value = 0.0;
    for action_entity in actions.iter() {
      if let Ok((action, action_state)) = move_actions.get(action_entity) {
        // Only show input when action is active
        if matches!(action_state, ActionState::Fired | ActionState::Ongoing) {
          move_value = **action;
        }
      }
    }

    if move_value.abs() > 0.01 {
      let input_direction = Vec2::new(move_value, 0.0).normalize_or_zero();
      let input_end = player_pos + input_direction * INPUT_LENGTH;
      gizmos.line_2d(player_pos, input_end, Color::srgb(0.0, 1.0, 0.0));
    }
  }
}
