use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

#[derive(Component)]
pub struct PlayerInput;

#[derive(Debug, InputAction)]
#[action_output(f32)]
pub struct Move;

#[derive(Debug, InputAction)]
#[action_output(bool)]
pub struct Fly;

#[derive(Debug, InputAction)]
#[action_output(f32)]
pub struct MoveVertical;
