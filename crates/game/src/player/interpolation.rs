use bevy::prelude::*;

use super::components::{CurrentPosition, Player, PlayerVisual, PreviousPosition, VisualPosition};

/// Runs in FixedFirst: Shift positions for interpolation
pub fn shift_positions(
  mut players: Query<(&mut PreviousPosition, &CurrentPosition), With<Player>>,
) {
  for (mut prev, current) in &mut players {
    prev.0 = current.0;
  }
}

/// Runs after Rapier writeback: Store new current position
pub fn store_current_position(
  mut players: Query<(&Transform, &mut CurrentPosition), With<Player>>,
) {
  for (transform, mut current) in &mut players {
    current.0 = transform.translation;
  }
}

/// Runs in Update: Interpolate the visual child entity for smooth rendering
pub fn interpolate_visual(
  mut physics_query: Query<
    (
      &Transform,
      &PreviousPosition,
      &CurrentPosition,
      &Children,
      &mut VisualPosition,
    ),
    With<Player>,
  >,
  mut visual_query: Query<&mut Transform, (With<PlayerVisual>, Without<Player>)>,
  fixed_time: Res<Time<Fixed>>,
) {
  let t = fixed_time.overstep_fraction();

  for (physics_transform, prev, current, children, mut visual_pos) in &mut physics_query {
    let interpolated_world = prev.0.lerp(current.0, t);

    // Store the interpolated position for camera to use
    visual_pos.0 = interpolated_world;

    for child in children.iter() {
      if let Ok(mut visual_transform) = visual_query.get_mut(child) {
        // Local offset = interpolated world position - parent world position
        visual_transform.translation = interpolated_world - physics_transform.translation;
      }
    }
  }
}
