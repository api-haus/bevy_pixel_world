//! Submersion state derived from liquid fraction.
//!
//! Applies a threshold to [`LiquidFractionState`] to produce binary
//! submerged/surfaced state and edge-detection for event emission.

use bevy::prelude::*;

use crate::pixel_world::pixel_awareness::LiquidFractionState;

/// Configuration for submersion threshold.
#[derive(Resource, Clone, Debug)]
pub struct SubmersionConfig {
  /// Fraction of body that must be in liquid to be considered "submerged".
  /// Default: 0.25 (25%).
  pub submersion_threshold: f32,
}

impl Default for SubmersionConfig {
  fn default() -> Self {
    Self {
      submersion_threshold: 0.25,
    }
  }
}

/// Marker component that enables submergence detection for a pixel body.
///
/// This is automatically added to all finalized pixel bodies.
#[derive(Component, Default)]
pub struct Submergent;

/// Tracks submersion state for a body.
///
/// Derived from [`LiquidFractionState`] by applying the submersion threshold.
#[derive(Component, Default)]
pub struct SubmersionState {
  /// Whether the body has crossed the submersion threshold.
  pub is_submerged: bool,
  /// Fraction of the body submerged in liquid (0.0 to 1.0).
  pub submerged_fraction: f32,
  /// World position of the center of buoyancy (center of submerged samples).
  pub submerged_center: Vec2,
  /// Previous frame's submerged state, for edge detection.
  pub(crate) previous_submerged: bool,
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

  /// Returns true if this is the first frame the body crossed out of
  /// submerged.
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

/// Derives [`SubmersionState`] from [`LiquidFractionState`] by applying
/// the submersion threshold.
pub fn derive_submersion_state(
  mut commands: Commands,
  config: Res<SubmersionConfig>,
  mut query: Query<(
    Entity,
    &LiquidFractionState,
    &Submergent,
    Option<&mut SubmersionState>,
  )>,
) {
  let threshold = config.submersion_threshold;

  for (entity, liquid, _, state) in query.iter_mut() {
    let is_submerged = liquid.liquid_fraction >= threshold;

    if let Some(mut state) = state {
      state.submerged_fraction = liquid.liquid_fraction;
      state.submerged_center = liquid.liquid_center;
      state.is_submerged = is_submerged;
      state.debug_liquid_samples = liquid.debug_liquid_samples;
      state.debug_total_samples = liquid.debug_total_samples;
    } else {
      commands.entity(entity).insert(SubmersionState {
        is_submerged,
        submerged_fraction: liquid.liquid_fraction,
        submerged_center: liquid.liquid_center,
        previous_submerged: false,
        debug_liquid_samples: liquid.debug_liquid_samples,
        debug_total_samples: liquid.debug_total_samples,
      });
    }
  }
}
