//! Optional buoyancy physics plugin for pixel bodies.
//!
//! This module provides liquid buoyancy simulation for pixel bodies.
//! Bodies marked with the [`Buoyant`] component will float or sink based
//! on their density relative to surrounding liquid pixels.
//!
//! # Usage
//!
//! ```ignore
//! use bevy_pixel_world::buoyancy::{PixelBuoyancyPlugin, Buoyant, BuoyancyConfig};
//!
//! app.add_plugins(PixelBuoyancyPlugin::default());
//!
//! // Mark a body as buoyant
//! commands.entity(body_entity).insert(Buoyant);
//! ```
//!
//! # Requirements
//!
//! This module requires either the `avian2d` or `rapier2d` feature to be
//! enabled.

mod force;

use bevy::prelude::*;
pub use force::compute_buoyancy_forces;

/// Configuration for buoyancy simulation.
#[derive(Resource, Clone, Debug)]
pub struct BuoyancyConfig {
  /// Size of the sample grid (NxN samples across body AABB).
  /// Higher values are more accurate but slower. Default: 4.
  pub sample_grid_size: u8,
  /// Multiplier for liquid density in force calculations.
  /// Adjust to tune buoyancy strength. Default: 0.1.
  pub liquid_density_scale: f32,
  /// Whether to apply rotational forces (torque) based on
  /// center of buoyancy offset. Default: true.
  pub torque_enabled: bool,
}

impl Default for BuoyancyConfig {
  fn default() -> Self {
    Self {
      sample_grid_size: 4,
      liquid_density_scale: 0.1,
      torque_enabled: true,
    }
  }
}

/// Marker component for bodies that respond to liquid buoyancy.
///
/// Add this component to a pixel body entity to enable buoyancy physics.
/// The body will float or sink based on its volume and the surrounding
/// liquid density.
#[derive(Component, Default)]
pub struct Buoyant;

/// Tracks buoyancy state for a body.
///
/// Automatically added to entities with [`Buoyant`] when they're sampled.
/// Contains information about how much of the body is submerged and where.
#[derive(Component, Default)]
pub struct BuoyancyState {
  /// Fraction of the body submerged in liquid (0.0 to 1.0).
  pub submerged_fraction: f32,
  /// World position of the center of buoyancy.
  pub submerged_center: Vec2,
}

/// Plugin for liquid buoyancy physics.
///
/// Adds systems that apply buoyancy forces to bodies marked with [`Buoyant`].
/// Submersion data comes from the submergence module's `SubmersionState`.
///
/// # Configuration
///
/// Pass a custom [`BuoyancyConfig`] to tune the simulation:
///
/// ```ignore
/// app.add_plugins(Buoyancy2dPlugin {
///     config: BuoyancyConfig {
///         sample_grid_size: 8,
///         liquid_density_scale: 0.15,
///         torque_enabled: true,
///     },
/// });
/// ```
#[derive(Default)]
pub struct Buoyancy2dPlugin {
  /// Configuration for the buoyancy simulation.
  pub config: BuoyancyConfig,
}

impl Buoyancy2dPlugin {
  /// Creates a new plugin with the given configuration.
  pub fn new(config: BuoyancyConfig) -> Self {
    Self { config }
  }
}

impl Plugin for Buoyancy2dPlugin {
  fn build(&self, app: &mut App) {
    app.insert_resource(self.config.clone());
    app.add_systems(Update, compute_buoyancy_forces);
  }
}

/// Deprecated alias for [`Buoyancy2dPlugin`].
#[deprecated(note = "Renamed to Buoyancy2dPlugin")]
pub type PixelBuoyancyPlugin = Buoyancy2dPlugin;
