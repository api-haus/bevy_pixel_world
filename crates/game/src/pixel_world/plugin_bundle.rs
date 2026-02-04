//! Convenience [`PluginGroup`] that adds all pixel world plugins.

use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;

use crate::pixel_world::PersistenceConfig;
use crate::pixel_world::PixelWorldPlugin;
use crate::pixel_world::bodies_plugin::PixelBodiesPlugin;
#[cfg(physics)]
use crate::pixel_world::buoyancy::SubmersionPhysicsConfig;
use crate::pixel_world::buoyancy::{Buoyancy2dPlugin, BuoyancyConfig, SubmersionConfig};
use crate::pixel_world::diagnostics::DiagnosticsPlugin;
use crate::pixel_world::pixel_awareness::{GridSampleConfig, PixelAwarenessPlugin};
use crate::pixel_world::world::streaming::CullingConfig;

/// Plugin group that adds [`PixelWorldPlugin`] and all optional sub-plugins
/// based on enabled features.
///
/// Persistence is always enabled - you must provide a save path.
///
/// # Usage
///
/// ```ignore
/// app.add_plugins(
///     PixelWorldFullBundle::new(PersistenceConfig::at("/path/to/save.save"))
///         .submersion(SubmersionConfig { submersion_threshold: 0.5, ..default() })
/// );
/// ```
pub struct PixelWorldFullBundle {
  /// Core world plugin (streaming, CA, rendering, persistence).
  pub world: PixelWorldPlugin,
  /// Pixel bodies plugin (spawning, collision, body persistence).
  pub bodies: PixelBodiesPlugin,
  /// Pixel awareness plugin (liquid detection sampling).
  pub awareness: PixelAwarenessPlugin,
  /// Buoyancy and submersion plugin (threshold, events, forces, physics).
  pub buoyancy: Buoyancy2dPlugin,
  /// Diagnostics plugin (frame time, simulation metrics).
  pub diagnostics: DiagnosticsPlugin,
}

impl PixelWorldFullBundle {
  /// Creates a new plugin bundle with the given persistence configuration.
  ///
  /// Persistence is always enabled. Provide the path where the world
  /// save file will be stored.
  pub fn new(persistence: PersistenceConfig) -> Self {
    Self {
      world: PixelWorldPlugin::new(persistence),
      bodies: PixelBodiesPlugin,
      awareness: PixelAwarenessPlugin::default(),
      buoyancy: Buoyancy2dPlugin::default(),
      diagnostics: DiagnosticsPlugin,
    }
  }

  /// Sets the culling configuration.
  pub fn culling(mut self, config: CullingConfig) -> Self {
    self.world = self.world.culling(config);
    self
  }

  /// Sets the grid sampling configuration for pixel awareness.
  pub fn awareness(mut self, config: GridSampleConfig) -> Self {
    self.awareness = PixelAwarenessPlugin::new(config);
    self
  }

  /// Sets the submersion threshold configuration.
  pub fn submersion(mut self, config: SubmersionConfig) -> Self {
    self.buoyancy = self.buoyancy.with_submersion(config);
    self
  }

  /// Sets the buoyancy configuration.
  pub fn buoyancy(mut self, config: BuoyancyConfig) -> Self {
    self.buoyancy = Buoyancy2dPlugin::new(config);
    self
  }

  /// Sets the submersion physics configuration.
  #[cfg(physics)]
  pub fn submersion_physics(mut self, config: SubmersionPhysicsConfig) -> Self {
    self.buoyancy = self.buoyancy.with_physics(config);
    self
  }
}

impl PluginGroup for PixelWorldFullBundle {
  fn build(self) -> PluginGroupBuilder {
    PluginGroupBuilder::start::<Self>()
      .add(self.world)
      .add(self.bodies)
      .add(self.awareness)
      .add(self.buoyancy)
      .add(self.diagnostics)
  }
}
