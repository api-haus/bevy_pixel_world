use bevy::prelude::*;

use crate::basic_persistence::BasicPersistencePlugin;
use crate::debug_camera::PixelDebugControllerCameraPlugin;
use crate::debug_controller::PixelDebugControllerPlugin;

pub struct CreativeModePlugins;

impl PluginGroup for CreativeModePlugins {
  fn build(self) -> bevy::app::PluginGroupBuilder {
    bevy::app::PluginGroupBuilder::start::<Self>()
      .add(PixelDebugControllerPlugin)
      .add(PixelDebugControllerCameraPlugin)
      .add(BasicPersistencePlugin)
  }
}
