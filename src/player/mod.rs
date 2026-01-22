pub mod components;
mod flight;
pub mod interpolation;
pub mod movement;
mod spawn;

#[cfg(test)]
mod tests;

use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
  fn build(&self, app: &mut App) {
    app
      .add_systems(Startup, spawn::spawn_player)
      // FixedFirst: Shift positions for interpolation
      .add_systems(FixedFirst, interpolation::shift_positions)
      .add_systems(
        FixedUpdate,
        (
          flight::process_locomotion_transitions, // Handle input (may enter/exit Flying)
          movement::handle_movement_input,        // Horizontal movement
          movement::apply_locomotion_physics,     // Gravity (Airborne only)
          movement::apply_velocity_to_controller, // Send to physics
        )
          .chain()
          .before(PhysicsSet::SyncBackend),
      )
      // Read physics output AFTER Rapier writeback (still in FixedUpdate)
      .add_systems(
        FixedUpdate,
        (
          movement::sync_ground_from_physics,
          interpolation::store_current_position,
        )
          .chain()
          .after(PhysicsSet::Writeback),
      )
      // Update: Interpolate the visual child for smooth rendering
      .add_systems(Update, interpolation::interpolate_visual);
  }
}
