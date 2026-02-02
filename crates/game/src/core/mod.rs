pub(crate) mod camera;
mod physics;

use bevy::prelude::*;
use bevy_crt::Crt2dPlugin;
pub use physics::GravityConfig;

use crate::pixel_world::{PixelCameraPlugin, PixelCameraSet};

pub struct CorePlugin;

impl Plugin for CorePlugin {
  fn build(&self, app: &mut App) {
    app
      .add_plugins(physics::PhysicsPlugin)
      .add_plugins(PixelCameraPlugin)
      .add_plugins(Crt2dPlugin)
      .add_systems(Startup, camera::setup_camera)
      .add_systems(PostUpdate, camera::camera_follow.before(PixelCameraSet));
  }
}
