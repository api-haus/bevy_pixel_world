//! Fixed-timestep interpolation for smooth character rendering.

use bevy::prelude::*;

use super::components::{InterpolationState, Player, PlayerVisual, VisualPosition};

/// FixedFirst: Shift previous = current before physics runs.
pub fn shift_interpolation_state(mut query: Query<&mut InterpolationState, With<Player>>) {
  for mut state in &mut query {
    state.previous = state.current;
  }
}

/// FixedUpdate (after Writeback): Store new physics position.
pub fn store_physics_position(
  mut query: Query<(&Transform, &mut InterpolationState), With<Player>>,
) {
  for (transform, mut state) in &mut query {
    state.current = transform.translation;
  }
}

/// Update: Calculate interpolated position using overstep fraction.
pub fn interpolate_visual_position(
  mut query: Query<(&InterpolationState, &mut VisualPosition), With<Player>>,
  fixed_time: Res<Time<Fixed>>,
) {
  let t = fixed_time.overstep_fraction();
  for (state, mut visual) in &mut query {
    visual.0 = state.previous.lerp(state.current, t);
  }
}

/// Update: Sync sprite transform to interpolated position.
///
/// The sprite is a separate root entity, so we set its position directly.
pub fn sync_sprite_to_visual(
  player_query: Query<&VisualPosition, With<Player>>,
  mut visual_query: Query<&mut Transform, With<PlayerVisual>>,
) {
  let Ok(visual_pos) = player_query.single() else {
    return;
  };
  let Ok(mut sprite_tf) = visual_query.single_mut() else {
    return;
  };
  sprite_tf.translation.x = visual_pos.0.x;
  sprite_tf.translation.y = visual_pos.0.y;
  // Keep z unchanged (render order)
}
