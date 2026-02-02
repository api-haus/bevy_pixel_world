use bevy::prelude::*;

use crate::pixel_world::basic_persistence::BasicPersistencePlugin;
use crate::pixel_world::debug_camera::PixelDebugControllerCameraPlugin;
use crate::pixel_world::debug_controller::PixelDebugControllerPlugin;

pub struct CreativeModePlugins;

impl PluginGroup for CreativeModePlugins {
  fn build(self) -> bevy::app::PluginGroupBuilder {
    bevy::app::PluginGroupBuilder::start::<Self>()
      .add(PixelDebugControllerPlugin)
      .add(PixelDebugControllerCameraPlugin)
      .add(BasicPersistencePlugin)
  }
}
