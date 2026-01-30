use bevy::prelude::*;

#[derive(Component)]
pub struct Player;

/// Marker for the visual child entity (sprite, camera target)
#[derive(Component)]
pub struct PlayerVisual;

#[derive(Component, Default)]
pub struct CharacterVelocity(pub Vec2);

/// Position at the START of the current physics frame (for interpolation)
#[derive(Component, Default)]
pub struct PreviousPosition(pub Vec3);

/// Position at the END of the current physics frame (for interpolation)
#[derive(Component, Default)]
pub struct CurrentPosition(pub Vec3);

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

/// The interpolated visual position for this frame.
/// Both sprite positioning and camera follow should use this exact value.
#[derive(Component, Default)]
pub struct VisualPosition(pub Vec3);
