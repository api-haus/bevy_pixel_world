use bevy::prelude::*;
use bevy_pixel_world::CollisionQueryPoint;
use bevy_rapier2d::prelude::*;

use super::components::{
  CharacterMovementConfig, CharacterVelocity, InterpolationState, LocomotionState, Player,
  PlayerVisual, VisualPosition,
};
use crate::config::ConfigLoaded;
use crate::input::{PlayerInput, player_input_actions};

/// Spawn player at a fixed position (non-editor mode)
#[cfg(not(feature = "editor"))]
pub fn spawn_player(
  mut commands: Commands,
  config: Res<ConfigLoaded>,
  asset_server: Res<AssetServer>,
) {
  let player = &config.player;
  let spawn_pos = Vec3::new(player.spawn_x, player.spawn_y, 0.0);
  spawn_player_entity(&mut commands, &config, &asset_server, spawn_pos);
}

/// Spawn player at the spawn point location (editor mode)
#[cfg(feature = "editor")]
pub fn spawn_player_at_spawn_point(
  mut commands: Commands,
  config: Res<ConfigLoaded>,
  asset_server: Res<AssetServer>,
  spawn_points: Query<&Transform, With<crate::editor::PlayerSpawnPoint>>,
) {
  let spawn_pos = spawn_points
    .iter()
    .next()
    .map(|t| t.translation)
    .unwrap_or(Vec3::new(0.0, 100.0, 0.0));

  info!("Spawning player at {:?}", spawn_pos);
  spawn_player_entity(&mut commands, &config, &asset_server, spawn_pos);
}

/// Despawn the player entity and its visual (editor mode)
#[cfg(feature = "editor")]
pub fn despawn_player(
  mut commands: Commands,
  players: Query<Entity, With<Player>>,
  visuals: Query<Entity, With<PlayerVisual>>,
) {
  for entity in &players {
    info!("Despawning player");
    commands.entity(entity).despawn();
  }
  for entity in &visuals {
    commands.entity(entity).despawn();
  }
}

fn spawn_player_entity(
  commands: &mut Commands,
  config: &ConfigLoaded,
  asset_server: &AssetServer,
  spawn_pos: Vec3,
) {
  let player = &config.player;
  let sprite_handle: Handle<Image> = asset_server.load(&player.sprite);

  // Rapier capsule_y uses half_height (cylinder part) and radius
  let half_height = player.collider_length / 2.0;

  // Physics entity - authoritative position for collision
  commands.spawn((
    Player,
    CollisionQueryPoint, // Required for PixelWorld terrain collision
    Transform::from_translation(spawn_pos),
    Visibility::default(),
    RigidBody::KinematicPositionBased,
    Collider::capsule_y(half_height, player.collider_radius),
    KinematicCharacterController {
      snap_to_ground: Some(CharacterLength::Absolute(player.snap_to_ground)),
      max_slope_climb_angle: player.max_slope_angle.to_radians(),
      min_slope_slide_angle: player.max_slope_angle.to_radians(),
      autostep: Some(CharacterAutostep {
        max_height: CharacterLength::Absolute(player.autostep_height),
        min_width: CharacterLength::Absolute(player.autostep_width),
        include_dynamic_bodies: false,
      }),
      ..default()
    },
    CharacterVelocity::default(),
    CharacterMovementConfig {
      walk_speed: player.walk_speed,
      acceleration: player.acceleration,
      air_acceleration: player.air_acceleration,
      flight_speed: player.flight_speed,
    },
    LocomotionState::Airborne, // Start airborne so gravity applies until landing
    InterpolationState::new(spawn_pos),
    VisualPosition(spawn_pos),
    PlayerInput,
    player_input_actions(),
  ));

  // Visual entity - separate root entity, follows VisualPosition directly
  commands.spawn((
    PlayerVisual,
    Sprite {
      image: sprite_handle.clone(),
      ..default()
    },
    bevy::sprite::Anchor(Vec2::new(
      player.sprite_pivot[0] - 0.5,
      player.sprite_pivot[1] - 0.5,
    )),
    Transform {
      scale: Vec3::splat(player.sprite_scale),
      translation: Vec3::new(spawn_pos.x, spawn_pos.y, 100.0),
      ..default()
    },
  ));
}
