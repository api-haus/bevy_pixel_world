//! Buoyancy force calculation and application.
//!
//! Computes buoyancy forces based on submersion state and applies them
//! to physics bodies.

#[cfg(physics)]
use bevy::prelude::*;

#[cfg(physics)]
use super::BuoyancyConfig;
#[cfg(physics)]
use super::submersion::SubmersionState;
#[cfg(physics)]
use crate::pixel_world::pixel_body::PixelBody;

/// Default gravity magnitude (matches typical physics engine defaults).
#[cfg(physics)]
const GRAVITY: f32 = 9.81 * 10.0; // Scaled for pixel world

/// Computes and applies buoyancy forces to submerged bodies.
#[cfg(physics)]
#[allow(clippy::type_complexity)]
pub fn compute_buoyancy_forces(
  config: Res<BuoyancyConfig>,
  mut bodies: Query<(
    &PixelBody,
    &GlobalTransform,
    &SubmersionState,
    &mut bevy_rapier2d::prelude::ExternalForce,
  )>,
) {
  for (body, transform, state, mut force) in bodies.iter_mut() {
    if state.submerged_fraction <= 0.0 {
      force.force = Vec2::ZERO;
      force.torque = 0.0;
      continue;
    }

    let body_volume = body.solid_count() as f32;
    let submerged_volume = body_volume * state.submerged_fraction;
    let buoyancy_magnitude = submerged_volume * GRAVITY * config.liquid_density_scale;

    force.force = Vec2::new(0.0, buoyancy_magnitude);

    if config.torque_enabled {
      let body_center = transform.translation().truncate();
      let buoyancy_center = state.submerged_center;
      let lever_arm = buoyancy_center - body_center;
      force.torque = lever_arm.x * buoyancy_magnitude;
    }
  }
}

/// No-op when physics is not enabled.
#[cfg(not(physics))]
pub fn compute_buoyancy_forces() {}
