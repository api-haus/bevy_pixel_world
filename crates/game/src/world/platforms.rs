use bevy::prelude::*;
use bevy_rapier2d::prelude::*;
use rand::{rngs::StdRng, Rng, SeedableRng};

use crate::config::ConfigLoaded;

pub fn spawn_platforms(mut commands: Commands, config: Res<ConfigLoaded>) {
  let ground = &config.ground;
  let platforms = &config.platforms;

  let mut rng = StdRng::seed_from_u64(platforms.seed);

  // Spawn ground platform
  // Rapier cuboid uses half-extents
  commands.spawn((
    Sprite {
      color: Color::srgb(ground.color[0], ground.color[1], ground.color[2]),
      custom_size: Some(Vec2::new(ground.width, ground.height)),
      ..default()
    },
    Transform::from_xyz(0.0, ground.y_position, 0.0),
    RigidBody::Fixed,
    Collider::cuboid(ground.width / 2.0, ground.height / 2.0),
  ));

  // Spawn random platforms
  for _ in 0..platforms.count {
    let width = rng.random_range(platforms.width_min..platforms.width_max);
    let height = platforms.height;
    let x = rng.random_range(platforms.x_min..platforms.x_max);
    let y = rng.random_range(platforms.y_min..platforms.y_max);

    commands.spawn((
      Sprite {
        color: Color::srgb(platforms.color[0], platforms.color[1], platforms.color[2]),
        custom_size: Some(Vec2::new(width, height)),
        ..default()
      },
      Transform::from_xyz(x, y, 0.0),
      RigidBody::Fixed,
      Collider::cuboid(width / 2.0, height / 2.0),
    ));
  }
}
