pub mod actions;
mod bindings;

pub use actions::{Fly, Move, MoveVertical, PlayerInput};
use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;
pub use bindings::player_input_actions;

pub struct InputPlugin;

impl Plugin for InputPlugin {
  fn build(&self, app: &mut App) {
    app
      .add_plugins(EnhancedInputPlugin)
      .add_input_context::<PlayerInput>();
  }
}
