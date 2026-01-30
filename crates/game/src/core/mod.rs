pub(crate) mod camera;
mod physics;

use bevy::prelude::*;
use bevy_pixel_world::{PixelCameraPlugin, VirtualCameraPlugin};
pub use camera::CameraTarget;
pub use physics::GravityConfig;

pub struct CorePlugin;

impl Plugin for CorePlugin {
  fn build(&self, app: &mut App) {
    app
      .init_resource::<camera::CameraSmoothness>()
      .add_plugins(physics::PhysicsPlugin)
      .add_plugins(PixelCameraPlugin)
      .add_plugins(VirtualCameraPlugin)
      .add_systems(Startup, camera::setup_camera)
      .add_systems(PostUpdate, camera::camera_follow);
  }
}
