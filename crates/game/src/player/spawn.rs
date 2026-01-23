use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

use super::components::{
    CharacterMovementConfig, CharacterVelocity, CurrentPosition, LocomotionState, Player,
    PlayerVisual, PreviousPosition,
};
use crate::config::ConfigLoaded;
use crate::core::CameraTarget;
use crate::input::{player_input_actions, PlayerInput};

pub fn spawn_player(mut commands: Commands, config: Res<ConfigLoaded>) {
  let player = &config.player;

  // Rapier capsule_y uses half_height (cylinder part) and radius
  // Avian2d capsule(radius, length) where length is the cylinder part
  let half_height = player.collider_length / 2.0;

  // Physics entity (parent) - authoritative position
  commands
    .spawn((
      Player,
      Transform::from_xyz(player.spawn_x, player.spawn_y, 0.0),
      Visibility::default(),
      RigidBody::KinematicPositionBased,
      Collider::capsule_y(half_height, player.collider_radius),
      KinematicCharacterController {
        snap_to_ground: Some(CharacterLength::Absolute(2.0)),
        ..default()
      },
    ))
    .insert((
      CharacterVelocity::default(),
      CharacterMovementConfig {
        walk_speed: player.walk_speed,
        acceleration: player.acceleration,
        air_acceleration: player.air_acceleration,
        flight_speed: player.flight_speed,
      },
      LocomotionState::Airborne, // Start airborne so gravity applies until landing
      // Positions for interpolation - initialize with spawn position
      PreviousPosition(Vec3::new(player.spawn_x, player.spawn_y, 0.0)),
      CurrentPosition(Vec3::new(player.spawn_x, player.spawn_y, 0.0)),
      PlayerInput,
      player_input_actions(),
    ))
    // Visual entity (child) - interpolated for smooth rendering
    .with_children(|parent| {
      parent.spawn((
        PlayerVisual,
        CameraTarget,
        Sprite {
          color: Color::srgb(player.color[0], player.color[1], player.color[2]),
          custom_size: Some(Vec2::new(player.width, player.height)),
          ..default()
        },
        Transform::default(),
      ));
    });
}
