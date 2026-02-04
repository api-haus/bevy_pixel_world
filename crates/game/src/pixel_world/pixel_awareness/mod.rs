//! Generic pixel awareness for pixel bodies.
//!
//! This module provides a reusable parallel pixel query engine. It samples
//! the world at grid points within each body's AABB and evaluates predicates
//! against adjacent pixels.
//!
//! Currently provides one concrete query: liquid fraction detection via
//! [`LiquidFractionState`].
//!
//! # Usage
//!
//! ```ignore
//! use crate::pixel_world::pixel_awareness::PixelAwarenessPlugin;
//!
//! app.add_plugins(PixelAwarenessPlugin::default());
//! ```

pub mod grid_sampler;
pub mod liquid;

use bevy::prelude::*;
pub use grid_sampler::GridSampleConfig;
pub use liquid::{LiquidFractionState, sample_liquid_fraction};

/// Plugin for pixel awareness (parallel pixel sampling queries).
///
/// Adds systems that sample pixel bodies against the world and produce
/// query results like [`LiquidFractionState`].
///
/// # Configuration
///
/// Pass a custom [`GridSampleConfig`] to tune sampling resolution:
///
/// ```ignore
/// app.add_plugins(PixelAwarenessPlugin {
///     config: GridSampleConfig { sample_grid_size: 8 },
/// });
/// ```
#[derive(Default)]
pub struct PixelAwarenessPlugin {
  /// Configuration for grid sampling resolution.
  pub config: GridSampleConfig,
}

impl PixelAwarenessPlugin {
  /// Creates a new plugin with the given configuration.
  pub fn new(config: GridSampleConfig) -> Self {
    Self { config }
  }
}

impl Plugin for PixelAwarenessPlugin {
  fn build(&self, app: &mut App) {
    app.insert_resource(self.config.clone());
    app.add_systems(Update, sample_liquid_fraction);
  }
}
