//! Convenience [`PluginGroup`] that adds all pixel world plugins.

use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;

use crate::PersistenceConfig;
use crate::PixelWorldPlugin;
use crate::bodies_plugin::PixelBodiesPlugin;
#[cfg(physics)]
use crate::buoyancy::SubmersionPhysicsConfig;
use crate::buoyancy::{Buoyancy2dPlugin, BuoyancyConfig, SubmersionConfig};
use crate::diagnostics::DiagnosticsPlugin;
use crate::pixel_awareness::{GridSampleConfig, PixelAwarenessPlugin};
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

impl Default for PixelWorldFullBundle {
  fn default() -> Self {
    Self {
      world: PixelWorldPlugin::default(),
      bodies: PixelBodiesPlugin,
      awareness: PixelAwarenessPlugin::default(),
      buoyancy: Buoyancy2dPlugin::default(),
      diagnostics: DiagnosticsPlugin,
    }
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
