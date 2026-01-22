use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

use super::actions::{Fly, Move, PlayerInput};

pub fn player_input_actions() -> impl Bundle {
  actions!(PlayerInput[
      (
          Action::<Move>::new(),
          Bindings::spawn((
              Bidirectional::ad_keys(),
              Bidirectional::left_right_arrow(),
          )),
      ),
      (
          Action::<Fly>::new(),
          bindings![KeyCode::Space],
      ),
  ])
}
