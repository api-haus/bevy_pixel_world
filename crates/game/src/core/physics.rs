use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

use crate::config::ConfigLoaded;

#[derive(Resource)]
pub struct GravityConfig {
  pub value: f32,
}

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
  fn build(&self, app: &mut App) {
    app
      .add_plugins(RapierPhysicsPlugin::<NoUserData>::default().with_length_unit(50.0))
      .add_systems(Startup, setup_gravity);
  }
}

fn setup_gravity(mut commands: Commands, config: Res<ConfigLoaded>) {
  commands.insert_resource(GravityConfig {
    value: config.physics.gravity,
  });
}
