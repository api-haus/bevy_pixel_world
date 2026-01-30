use bevy::log::debug;
use bevy::prelude::*;
use bevy_yoleck::prelude::*;
use bevy_yoleck::vpeol::prelude::*;
use serde::{Deserialize, Serialize};

/// Marker component for the player spawn point.
/// The player entity will be spawned at this location when entering play mode.
#[derive(Component, Default)]
pub struct PlayerSpawnPoint;

/// Spawn point position data
#[derive(Clone, PartialEq, Serialize, Deserialize, Component, YoleckComponent, Default)]
pub struct SpawnPointData {
  pub x: f32,
  pub y: f32,
}

pub fn register(app: &mut App) {
  app.add_yoleck_entity_type(
    YoleckEntityType::new("PlayerSpawnPoint")
      .with::<Vpeol2dPosition>()
      .with::<SpawnPointData>(),
  );
  app.add_systems(YoleckSchedule::Populate, populate_spawn_point);
}

fn populate_spawn_point(mut populate: YoleckPopulate<(&Vpeol2dPosition, &SpawnPointData)>) {
  populate.populate(|_ctx, mut cmd, (vpeol_pos, data)| {
    // Use SpawnPointData if non-zero, otherwise fall back to Vpeol2dPosition
    let pos = if data.x != 0.0 || data.y != 0.0 {
      Vec2::new(data.x, data.y)
    } else {
      vpeol_pos.0
    };
    debug!(
      "Populating spawn point at {:?} (vpeol={:?}, data=({}, {}))",
      pos, vpeol_pos.0, data.x, data.y
    );
    cmd.insert((
      PlayerSpawnPoint,
      Transform::from_translation(pos.extend(0.0)),
      Sprite {
        color: Color::srgba(0.0, 1.0, 0.0, 0.5),
        custom_size: Some(Vec2::new(30.0, 50.0)),
        ..default()
      },
    ));
  });
}
