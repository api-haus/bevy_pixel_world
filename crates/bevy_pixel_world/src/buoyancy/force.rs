//! Buoyancy force calculation and application.
//!
//! Computes buoyancy forces based on submersion state and applies them
//! to physics bodies.

use bevy::prelude::*;

use super::BuoyancyConfig;
#[cfg(not(feature = "submergence"))]
use super::BuoyancyState;
use crate::pixel_body::PixelBody;
#[cfg(feature = "submergence")]
use crate::submergence::SubmersionState;

/// Default gravity magnitude (matches typical physics engine defaults).
const GRAVITY: f32 = 9.81 * 10.0; // Scaled for pixel world

/// Computes and applies buoyancy forces to submerged bodies.
///
/// Uses Archimedes' principle: the buoyancy force equals the weight of
/// the displaced fluid. The force is applied at the center of buoyancy,
/// which may create torque if it doesn't align with the center of mass.
#[allow(clippy::type_complexity)]
pub fn compute_buoyancy_forces(
  config: Res<BuoyancyConfig>,
  #[cfg(all(feature = "avian2d", not(feature = "submergence")))] mut bodies: Query<(
    &PixelBody,
    &GlobalTransform,
    &BuoyancyState,
    &mut avian2d::dynamics::rigid_body::forces::ConstantForce,
    Option<&mut avian2d::dynamics::rigid_body::forces::ConstantTorque>,
  )>,
  #[cfg(all(feature = "avian2d", feature = "submergence"))] mut bodies: Query<(
    &PixelBody,
    &GlobalTransform,
    &SubmersionState,
    &mut avian2d::dynamics::rigid_body::forces::ConstantForce,
    Option<&mut avian2d::dynamics::rigid_body::forces::ConstantTorque>,
  )>,
  #[cfg(all(
    feature = "rapier2d",
    not(feature = "avian2d"),
    not(feature = "submergence")
  ))]
  mut bodies: Query<(
    &PixelBody,
    &GlobalTransform,
    &BuoyancyState,
    &mut bevy_rapier2d::prelude::ExternalForce,
  )>,
  #[cfg(all(
    feature = "rapier2d",
    not(feature = "avian2d"),
    feature = "submergence"
  ))]
  mut bodies: Query<(
    &PixelBody,
    &GlobalTransform,
    &SubmersionState,
    &mut bevy_rapier2d::prelude::ExternalForce,
  )>,
) {
  #[cfg(feature = "avian2d")]
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

  #[cfg(all(feature = "rapier2d", not(feature = "avian2d")))]
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
