use bevy::prelude::*;
use bevy_pixel_world::CollisionQueryPoint;
use bevy_rapier2d::prelude::*;

use super::components::{
  CharacterMovementConfig, CharacterVelocity, CurrentPosition, LocomotionState, Player,
  PlayerVisual, PreviousPosition, VisualPosition,
};
use crate::config::ConfigLoaded;
use crate::core::CameraTarget;
use crate::input::{PlayerInput, player_input_actions};

/// Spawn player at a fixed position (non-editor mode)
#[cfg(not(feature = "editor"))]
pub fn spawn_player(mut commands: Commands, config: Res<ConfigLoaded>) {
  let player = &config.player;
  let spawn_pos = Vec3::new(player.spawn_x, player.spawn_y, 0.0);
  spawn_player_entity(&mut commands, &config, spawn_pos);
}

/// Spawn player at the spawn point location (editor mode)
#[cfg(feature = "editor")]
pub fn spawn_player_at_spawn_point(
  mut commands: Commands,
  config: Res<ConfigLoaded>,
  spawn_points: Query<&Transform, With<crate::editor::PlayerSpawnPoint>>,
) {
  let spawn_pos = spawn_points
    .iter()
    .next()
    .map(|t| t.translation)
    .unwrap_or(Vec3::new(0.0, 100.0, 0.0));

  info!("Spawning player at {:?}", spawn_pos);
  spawn_player_entity(&mut commands, &config, spawn_pos);
}

/// Despawn the player entity (editor mode)
#[cfg(feature = "editor")]
pub fn despawn_player(mut commands: Commands, players: Query<Entity, With<Player>>) {
  for entity in &players {
    info!("Despawning player");
    commands.entity(entity).despawn();
  }
}

fn spawn_player_entity(commands: &mut Commands, config: &ConfigLoaded, spawn_pos: Vec3) {
  let player = &config.player;

  // Rapier capsule_y uses half_height (cylinder part) and radius
  let half_height = player.collider_length / 2.0;

  // Physics entity (parent) - authoritative position
  commands
    .spawn((
      Player,
      CollisionQueryPoint, // Required for PixelWorld terrain collision
      Transform::from_translation(spawn_pos),
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
      PreviousPosition(spawn_pos),
      CurrentPosition(spawn_pos),
      VisualPosition(spawn_pos),
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
