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
use crate::pixel_body::PixelBody;

/// Default gravity magnitude (matches typical physics engine defaults).
#[cfg(physics)]
const GRAVITY: f32 = 9.81 * 10.0; // Scaled for pixel world

/// Computes and applies buoyancy forces to submerged bodies (avian2d).
#[cfg(feature = "avian2d")]
#[allow(clippy::type_complexity)]
pub fn compute_buoyancy_forces(
  config: Res<BuoyancyConfig>,
  mut bodies: Query<(
    &PixelBody,
    &GlobalTransform,
    &SubmersionState,
    &mut avian2d::dynamics::rigid_body::forces::ConstantForce,
    Option<&mut avian2d::dynamics::rigid_body::forces::ConstantTorque>,
  )>,
) {
  for (body, transform, state, mut force, torque) in bodies.iter_mut() {
    if state.submerged_fraction <= 0.0 {
      *force = avian2d::dynamics::rigid_body::forces::ConstantForce::new(0.0, 0.0);
      if let Some(mut torque) = torque {
        *torque = avian2d::dynamics::rigid_body::forces::ConstantTorque(0.0);
      }
      continue;
    }

    let body_volume = body.solid_count() as f32;
    let submerged_volume = body_volume * state.submerged_fraction;
    let buoyancy_magnitude = submerged_volume * GRAVITY * config.liquid_density_scale;

    *force = avian2d::dynamics::rigid_body::forces::ConstantForce::new(0.0, buoyancy_magnitude);

    if config.torque_enabled {
      if let Some(mut torque_component) = torque {
        let body_center = transform.translation().truncate();
        let buoyancy_center = state.submerged_center;
        let lever_arm = buoyancy_center - body_center;
        let torque_magnitude = lever_arm.x * buoyancy_magnitude;
        *torque_component = avian2d::dynamics::rigid_body::forces::ConstantTorque(torque_magnitude);
      }
    }
  }
}

/// Computes and applies buoyancy forces to submerged bodies (rapier2d).
#[cfg(all(feature = "rapier2d", not(feature = "avian2d")))]
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

/// No-op when neither physics engine is enabled.
#[cfg(not(physics))]
pub fn compute_buoyancy_forces() {}
