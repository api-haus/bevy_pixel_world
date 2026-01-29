pub mod components;
mod flight;
pub mod interpolation;
pub mod movement;
mod spawn;
mod spawn_body;

#[cfg(test)]
mod tests;

use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
  fn build(&self, app: &mut App) {
    // Editor mode: spawn/despawn player based on GameMode
    #[cfg(feature = "editor")]
    {
      use crate::editor::GameMode;
      app
        .add_systems(
          OnEnter(GameMode::Playing),
          spawn::spawn_player_at_spawn_point,
        )
        .add_systems(OnExit(GameMode::Playing), spawn::despawn_player);
    }

    // Non-editor mode: spawn player on startup
    #[cfg(not(feature = "editor"))]
    app.add_systems(Startup, spawn::spawn_player);

    // Player systems - always registered, but only run when player exists
    app
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
      .add_systems(
        Update,
        (
          interpolation::interpolate_visual,
          spawn_body::spawn_body_on_input,
          spawn_body::tag_new_bodies_as_bombs,
        ),
      );
  }
}
