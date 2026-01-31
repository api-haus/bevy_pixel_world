pub mod components;
mod flight;
pub mod interpolation;
pub mod movement;
mod palettize;
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

    // FixedFirst: Shift interpolation state
    app.add_systems(FixedFirst, interpolation::shift_interpolation_state);

    // FixedUpdate: Physics input
    app
      .add_systems(
        FixedUpdate,
        (
          flight::process_locomotion_transitions,
          movement::handle_movement_input,
          movement::apply_locomotion_physics,
          movement::apply_velocity_to_controller,
        )
          .chain()
          .before(PhysicsSet::SyncBackend),
      )
      .add_systems(
        FixedUpdate,
        (
          movement::sync_ground_from_physics,
          interpolation::store_physics_position,
        )
          .chain()
          .after(PhysicsSet::Writeback),
      );

    // Update: Calculate interpolated position and sync sprite
    app.add_systems(
      Update,
      (
        interpolation::interpolate_visual_position,
        interpolation::sync_sprite_to_visual,
      )
        .chain(),
    );

    // Misc systems
    app.add_systems(
      Update,
      (
        spawn_body::spawn_body_on_input,
        spawn_body::tag_new_bodies_as_bombs,
        palettize::palettize_player_sprite,
      ),
    );
  }
}
