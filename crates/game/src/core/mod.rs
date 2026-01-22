mod camera;
mod physics;

use bevy::prelude::*;
pub use camera::{CameraTarget, GameCamera};
pub use physics::GravityConfig;

pub struct CorePlugin;

impl Plugin for CorePlugin {
  fn build(&self, app: &mut App) {
    app
      .insert_resource(ClearColor(Color::BLACK))
      .init_resource::<camera::CameraSmoothness>()
      .add_plugins(physics::PhysicsPlugin)
      .add_systems(Startup, camera::setup_camera)
      .add_systems(PostUpdate, camera::camera_follow);
  }
}
