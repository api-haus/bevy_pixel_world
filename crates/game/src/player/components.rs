use bevy::prelude::*;

#[derive(Component)]
pub struct Player;

/// Marker for the visual child entity (sprite, camera target)
#[derive(Component)]
pub struct PlayerVisual;

#[derive(Component, Default)]
pub struct CharacterVelocity(pub Vec2);

#[derive(Component)]
pub struct CharacterMovementConfig {
  pub walk_speed: f32,
  pub acceleration: f32,
  pub air_acceleration: f32,
  pub flight_speed: f32,
}

#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocomotionState {
  #[default]
  Grounded,
  Airborne,
  Flying,
}

/// Stores physics positions for fixed-timestep interpolation.
#[derive(Component)]
pub struct InterpolationState {
  pub previous: Vec3,
  pub current: Vec3,
}

impl InterpolationState {
  pub fn new(position: Vec3) -> Self {
    Self {
      previous: position,
      current: position,
    }
  }
}

/// The interpolated visual position for this frame.
#[derive(Component, Default)]
pub struct VisualPosition(pub Vec3);
