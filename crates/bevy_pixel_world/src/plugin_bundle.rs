//! Convenience [`PluginGroup`] that adds all pixel world plugins.

use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;

use crate::PersistenceConfig;
use crate::PixelWorldPlugin;
use crate::bodies_plugin::PixelBodiesPlugin;
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
use crate::buoyancy::{Buoyancy2dPlugin, BuoyancyConfig};
use crate::diagnostics::DiagnosticsPlugin;
use crate::submergence::{PixelAwarenessPlugin, SubmersionConfig};
use crate::world::streaming::CullingConfig;

/// Plugin group that adds [`PixelWorldPlugin`] and all optional sub-plugins
/// based on enabled features.
///
/// # Usage
///
/// ```ignore
/// app.add_plugins(
///     PixelWorldFullBundle::new("my_game")
///         .load("world")
///         .awareness(SubmersionConfig { sample_grid_size: 8, ..default() })
/// );
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
  /// Diagnostics plugin (frame time, simulation metrics).
  pub diagnostics: DiagnosticsPlugin,
}

impl PixelWorldFullBundle {
  /// Creates a bundle with persistence enabled for the given app name.
  pub fn new(app_name: impl Into<String>) -> Self {
    Self {
      world: PixelWorldPlugin::with_persistence(app_name),
      ..Default::default()
    }
  }

  /// Sets the persistence configuration.
  pub fn persistence(mut self, config: PersistenceConfig) -> Self {
    self.world = self.world.persistence(config);
    self
  }

  /// Sets the save name to load.
  pub fn load(mut self, save_name: &str) -> Self {
    self.world = self.world.load(save_name);
    self
  }

  /// Sets the culling configuration.
  pub fn culling(mut self, config: CullingConfig) -> Self {
    self.world = self.world.culling(config);
    self
  }

  /// Sets the submersion awareness configuration.
  pub fn awareness(mut self, config: SubmersionConfig) -> Self {
    self.awareness = PixelAwarenessPlugin::new(config);
    self
  }

  /// Sets the buoyancy configuration.
  #[cfg(any(feature = "avian2d", feature = "rapier2d"))]
  pub fn buoyancy(mut self, config: BuoyancyConfig) -> Self {
    self.buoyancy = Buoyancy2dPlugin::new(config);
    self
  }
}

impl Default for PixelWorldFullBundle {
  fn default() -> Self {
    Self {
      world: PixelWorldPlugin::default(),
      bodies: PixelBodiesPlugin,
      awareness: PixelAwarenessPlugin::default(),
      #[cfg(any(feature = "avian2d", feature = "rapier2d"))]
      buoyancy: Buoyancy2dPlugin::default(),
      diagnostics: DiagnosticsPlugin,
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

    builder.add(self.diagnostics)
  }
}
