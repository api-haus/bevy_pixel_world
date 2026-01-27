//! Convenience [`PluginGroup`] that adds all pixel world plugins.

use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;

use crate::PixelWorldPlugin;
use crate::bodies_plugin::PixelBodiesPlugin;
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
use crate::buoyancy::Buoyancy2dPlugin;
use crate::submergence::PixelAwarenessPlugin;

/// Plugin group that adds [`PixelWorldPlugin`] and all optional sub-plugins
/// based on enabled features.
///
/// # Usage
///
/// ```ignore
/// app.add_plugins(PixelWorldFullBundle {
///     world: PixelWorldPlugin::with_persistence("my_game"),
///     ..default()
/// });
/// ```
pub struct PixelWorldFullBundle {
  /// Core world plugin (streaming, CA, rendering, persistence).
  pub world: PixelWorldPlugin,
  /// Pixel bodies plugin (spawning, collision, body persistence).
  pub bodies: PixelBodiesPlugin,
  /// Submergence awareness plugin (liquid detection, events).
  pub awareness: PixelAwarenessPlugin,
  /// Buoyancy physics plugin (forces, torque).
  #[cfg(any(feature = "avian2d", feature = "rapier2d"))]
  pub buoyancy: Buoyancy2dPlugin,
}

impl Default for PixelWorldFullBundle {
  fn default() -> Self {
    Self {
      world: PixelWorldPlugin::default(),
      bodies: PixelBodiesPlugin,
      awareness: PixelAwarenessPlugin::default(),
      #[cfg(any(feature = "avian2d", feature = "rapier2d"))]
      buoyancy: Buoyancy2dPlugin::default(),
    }
  }
}

impl PluginGroup for PixelWorldFullBundle {
  fn build(self) -> PluginGroupBuilder {
    let builder = PluginGroupBuilder::start::<Self>()
      .add(self.world)
      .add(self.bodies)
      .add(self.awareness);

    #[cfg(any(feature = "avian2d", feature = "rapier2d"))]
    let builder = builder.add(self.buoyancy);

    builder
  }
}
