//! Physics modification based on submersion state.
//!
//! Adjusts gravity scale, linear damping, and angular damping for submerged
//! pixel bodies.

use bevy::prelude::*;

use super::submersion::SubmersionState;

/// Configuration for submersion physics effects.
#[derive(Resource, Clone, Debug)]
pub struct SubmersionPhysicsConfig {
  /// Gravity scale when fully submerged (0.0 = no gravity, 1.0 = full
  /// gravity). Default: 0.3 (reduced gravity underwater).
  pub submerged_gravity_scale: f32,
  /// Linear damping when fully submerged.
  /// Default: 3.0 (significant drag in water).
  pub submerged_linear_damping: f32,
  /// Angular damping when fully submerged.
  /// Default: 2.0 (rotational drag in water).
  pub submerged_angular_damping: f32,
  /// Gravity scale when not submerged.
  /// Default: 1.0 (normal gravity).
  pub surface_gravity_scale: f32,
  /// Linear damping when not submerged.
  /// Default: 0.0 (no air resistance).
  pub surface_linear_damping: f32,
  /// Angular damping when not submerged.
  /// Default: 0.0 (no rotational resistance).
  pub surface_angular_damping: f32,
}

impl Default for SubmersionPhysicsConfig {
  fn default() -> Self {
    Self {
      submerged_gravity_scale: 0.3,
      submerged_linear_damping: 3.0,
      submerged_angular_damping: 2.0,
      surface_gravity_scale: 1.0,
      surface_linear_damping: 0.0,
      surface_angular_damping: 0.0,
    }
  }
}

impl SubmersionPhysicsConfig {
  /// Creates a config with the given submerged values.
  pub fn new(gravity_scale: f32, linear_damping: f32, angular_damping: f32) -> Self {
    Self {
      submerged_gravity_scale: gravity_scale,
      submerged_linear_damping: linear_damping,
      submerged_angular_damping: angular_damping,
      ..Default::default()
    }
  }

  /// Interpolates physics values based on submersion fraction.
  fn lerp(&self, t: f32) -> (f32, f32, f32) {
    let gravity =
      self.surface_gravity_scale + t * (self.submerged_gravity_scale - self.surface_gravity_scale);
    let linear = self.surface_linear_damping
      + t * (self.submerged_linear_damping - self.surface_linear_damping);
    let angular = self.surface_angular_damping
      + t * (self.submerged_angular_damping - self.surface_angular_damping);
    (gravity, linear, angular)
  }
}

/// Applies physics modifications based on submersion state.
pub fn apply_submersion_physics(
  config: Res<SubmersionPhysicsConfig>,
  mut bodies: Query<(
    &SubmersionState,
    &mut bevy_rapier2d::prelude::GravityScale,
    &mut bevy_rapier2d::prelude::Damping,
  )>,
) {
  for (state, mut gravity, mut damping) in bodies.iter_mut() {
    let (g, l, a) = config.lerp(state.submerged_fraction);
    gravity.0 = g;
    damping.linear_damping = l;
    damping.angular_damping = a;
  }
}
