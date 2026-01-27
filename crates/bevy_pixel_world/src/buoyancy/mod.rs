//! Buoyancy and submersion physics for pixel bodies.
//!
//! This module provides:
//! - **Submersion detection**: Threshold-based submerged/surfaced state derived
//!   from [`LiquidFractionState`](crate::pixel_awareness::LiquidFractionState),
//!   with edge-detection events.
//! - **Buoyancy forces**: Archimedes-principle forces for bodies marked
//!   [`Buoyant`].
//!
//! # Usage
//!
//! ```ignore
//! use bevy_pixel_world::buoyancy::{Buoyancy2dPlugin, BuoyancyConfig};
//!
//! app.add_plugins(Buoyancy2dPlugin::default());
//!
//! // Mark a body as buoyant
//! commands.entity(body_entity).insert(Buoyant);
//! ```
//!
//! # Requirements
//!
//! This module requires either the `avian2d` or `rapier2d` feature to be
//! enabled.

pub mod events;
mod force;
#[cfg(physics)]
pub mod physics;
pub mod submersion;

use bevy::prelude::*;
pub use events::emit_submersion_events;
pub use force::compute_buoyancy_forces;
#[cfg(physics)]
pub use physics::{SubmersionPhysicsConfig, apply_submersion_physics};
pub use submersion::{
  Submerged, Submergent, SubmersionConfig, SubmersionState, Surfaced, derive_submersion_state,
};

use crate::pixel_awareness::sample_liquid_fraction;

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

/// Plugin for liquid buoyancy and submersion physics.
///
/// Adds systems for:
/// - Deriving submersion state from liquid fraction (threshold + events)
/// - Applying buoyancy forces to [`Buoyant`] bodies
/// - Modifying gravity/damping for submerged bodies (when physics enabled)
///
/// Requires [`PixelAwarenessPlugin`](crate::pixel_awareness::PixelAwarenessPlugin)
/// to be added first.
///
/// # Configuration
///
/// ```ignore
/// app.add_plugins(Buoyancy2dPlugin {
///     config: BuoyancyConfig {
///         liquid_density_scale: 0.15,
///         ..default()
///     },
///     ..default()
/// });
/// ```
#[derive(Default)]
pub struct Buoyancy2dPlugin {
  /// Configuration for the buoyancy simulation.
  pub config: BuoyancyConfig,
  /// Configuration for submersion threshold.
  pub submersion: SubmersionConfig,
  /// Configuration for physics effects (gravity, damping).
  #[cfg(physics)]
  pub physics: SubmersionPhysicsConfig,
}

impl Buoyancy2dPlugin {
  /// Creates a new plugin with the given configuration.
  pub fn new(config: BuoyancyConfig) -> Self {
    Self {
      config,
      ..Default::default()
    }
  }

  /// Sets the submersion configuration.
  pub fn with_submersion(mut self, config: SubmersionConfig) -> Self {
    self.submersion = config;
    self
  }

  /// Sets the physics configuration.
  #[cfg(physics)]
  pub fn with_physics(mut self, physics: SubmersionPhysicsConfig) -> Self {
    self.physics = physics;
    self
  }
}

impl Plugin for Buoyancy2dPlugin {
  fn build(&self, app: &mut App) {
    app.insert_resource(self.config.clone());
    app.insert_resource(self.submersion.clone());
    app.add_message::<Submerged>();
    app.add_message::<Surfaced>();

    app.add_systems(
      Update,
      (derive_submersion_state, emit_submersion_events)
        .chain()
        .after(sample_liquid_fraction),
    );

    app.add_systems(
      Update,
      compute_buoyancy_forces.after(derive_submersion_state),
    );

    #[cfg(physics)]
    {
      app.insert_resource(self.physics.clone());
      app.add_systems(
        Update,
        apply_submersion_physics.after(derive_submersion_state),
      );
    }
  }
}
