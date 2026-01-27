//! Submergence detection for pixel bodies.
//!
//! This module provides automatic liquid awareness for pixel bodies. Bodies
//! with the [`Submergent`] marker have their submersion state tracked and
//! emit events when crossing the submersion threshold.
//!
//! # Usage
//!
//! ```ignore
//! use bevy_pixel_world::submergence::{PixelSubmergencePlugin, SubmersionConfig};
//!
//! app.add_plugins(PixelSubmergencePlugin::default());
//! ```
//!
//! All pixel bodies automatically gain the [`Submergent`] marker on spawn.

mod events;
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
mod physics;
mod sample;

use bevy::prelude::*;
pub use events::emit_submersion_events;
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
pub use physics::{SubmersionPhysicsConfig, apply_submersion_physics};
pub use sample::sample_submersion;

/// Configuration for submersion detection.
#[derive(Resource, Clone, Debug)]
pub struct SubmersionConfig {
  /// Size of the sample grid (NxN samples across body AABB).
  /// Higher values are more accurate but slower. Default: 4.
  pub sample_grid_size: u8,
  /// Fraction of body that must be submerged to be considered "submerged".
  /// Default: 0.25 (25%).
  pub submersion_threshold: f32,
}

impl Default for SubmersionConfig {
  fn default() -> Self {
    Self {
      sample_grid_size: 4,
      submersion_threshold: 0.25,
    }
  }
}

/// Marker component that enables submergence detection for a pixel body.
///
/// This is automatically added to all finalized pixel bodies when the
/// `submergence` feature is enabled.
#[derive(Component, Default)]
pub struct Submergent;

/// Tracks submersion state for a body.
///
/// Automatically added to entities with [`Submergent`] when they're first
/// sampled. Contains information about how much of the body is submerged.
#[derive(Component, Default)]
pub struct SubmersionState {
  /// Whether the body has crossed the submersion threshold.
  pub is_submerged: bool,
  /// Fraction of the body submerged in liquid (0.0 to 1.0).
  pub submerged_fraction: f32,
  /// World position of the center of buoyancy (center of submerged samples).
  pub submerged_center: Vec2,
  /// Previous frame's submerged state, for edge detection.
  previous_submerged: bool,
  /// Debug: number of sample points that hit liquid.
  pub debug_liquid_samples: u32,
  /// Debug: total number of sample points that hit solid body pixels.
  pub debug_total_samples: u32,
}

impl SubmersionState {
  /// Returns true if this is the first frame the body crossed into submerged.
  pub fn just_submerged(&self) -> bool {
    self.is_submerged && !self.previous_submerged
  }

  /// Returns true if this is the first frame the body crossed out of submerged.
  pub fn just_surfaced(&self) -> bool {
    !self.is_submerged && self.previous_submerged
  }
}

/// Message sent when a body crosses the submersion threshold into liquid.
#[derive(bevy::prelude::Message)]
pub struct Submerged {
  /// The entity that became submerged.
  pub entity: Entity,
  /// Current fraction of the body submerged.
  pub submerged_fraction: f32,
}

/// Message sent when a body crosses the submersion threshold out of liquid.
#[derive(bevy::prelude::Message)]
pub struct Surfaced {
  /// The entity that surfaced.
  pub entity: Entity,
}

/// Plugin for submergence detection (pixel awareness).
///
/// Adds systems that sample submersion state and emit threshold-crossing
/// events. When physics features are enabled, also modifies gravity and
/// damping based on submersion.
///
/// # Configuration
///
/// Pass a custom [`SubmersionConfig`] to tune detection:
///
/// ```ignore
/// app.add_plugins(PixelAwarenessPlugin {
///     config: SubmersionConfig {
///         sample_grid_size: 8,
///         submersion_threshold: 0.5,
///     },
///     ..default()
/// });
/// ```
#[derive(Default)]
pub struct PixelAwarenessPlugin {
  /// Configuration for submersion detection.
  pub config: SubmersionConfig,
  /// Configuration for physics effects (gravity, damping).
  #[cfg(any(feature = "avian2d", feature = "rapier2d"))]
  pub physics: SubmersionPhysicsConfig,
}

impl PixelAwarenessPlugin {
  /// Creates a new plugin with the given configuration.
  pub fn new(config: SubmersionConfig) -> Self {
    Self {
      config,
      #[cfg(any(feature = "avian2d", feature = "rapier2d"))]
      physics: SubmersionPhysicsConfig::default(),
    }
  }

  /// Sets the physics configuration.
  #[cfg(any(feature = "avian2d", feature = "rapier2d"))]
  pub fn with_physics(mut self, physics: SubmersionPhysicsConfig) -> Self {
    self.physics = physics;
    self
  }
}

impl Plugin for PixelAwarenessPlugin {
  fn build(&self, app: &mut App) {
    app.insert_resource(self.config.clone());
    app.add_message::<Submerged>();
    app.add_message::<Surfaced>();
    app.add_systems(Update, (sample_submersion, emit_submersion_events).chain());

    #[cfg(any(feature = "avian2d", feature = "rapier2d"))]
    {
      app.insert_resource(self.physics.clone());
      app.add_systems(Update, apply_submersion_physics.after(sample_submersion));
    }
  }
}

/// Deprecated alias for [`PixelAwarenessPlugin`].
#[deprecated(note = "Renamed to PixelAwarenessPlugin")]
pub type PixelSubmergencePlugin = PixelAwarenessPlugin;
