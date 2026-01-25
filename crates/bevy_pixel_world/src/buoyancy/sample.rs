//! Submersion sampling for buoyancy calculations.
//!
//! Samples the world at grid points within a body's AABB to determine
//! how much of the body is submerged in liquid.

use bevy::prelude::*;

use super::{BuoyancyConfig, BuoyancyState, Buoyant};
use crate::coords::WorldPos;
use crate::material::{Materials, PhysicsState};
use crate::pixel_body::{PixelBody, compute_world_aabb};
use crate::world::PixelWorld;

/// Samples submersion for all buoyant pixel bodies.
///
/// Creates an NxN sample grid across each body's AABB and queries the world
/// for liquid pixels. Updates `BuoyancyState` with the fraction submerged
/// and the center of buoyancy.
pub fn sample_submersion(
  mut commands: Commands,
  worlds: Query<&PixelWorld>,
  materials: Res<Materials>,
  config: Res<BuoyancyConfig>,
  mut bodies: Query<(
    Entity,
    &PixelBody,
    &GlobalTransform,
    &Buoyant,
    Option<&mut BuoyancyState>,
  )>,
) {
  let Ok(world) = worlds.single() else {
    return;
  };

  let grid_size = config.sample_grid_size as usize;

  for (entity, body, transform, _, state) in bodies.iter_mut() {
    let aabb = compute_world_aabb(body, transform);

    // Calculate sample step sizes
    let step_x = aabb.width as f32 / grid_size as f32;
    let step_y = aabb.height as f32 / grid_size as f32;

    let mut liquid_samples = 0;
    let mut total_samples = 0;
    let mut liquid_center_sum = Vec2::ZERO;

    // Sample grid points
    for gy in 0..grid_size {
      for gx in 0..grid_size {
        let sample_x = aabb.x as f32 + (gx as f32 + 0.5) * step_x;
        let sample_y = aabb.y as f32 + (gy as f32 + 0.5) * step_y;
        let sample_pos = WorldPos::new(sample_x as i64, sample_y as i64);

        // Check if this sample point is within the body's shape
        let world_point = Vec3::new(sample_x, sample_y, 0.0);
        let local_point = transform.affine().inverse().transform_point3(world_point);
        let local_x = (local_point.x - body.origin.x as f32).floor() as i32;
        let local_y = (local_point.y - body.origin.y as f32).floor() as i32;

        if local_x < 0
          || local_x >= body.width() as i32
          || local_y < 0
          || local_y >= body.height() as i32
        {
          continue;
        }

        if !body.is_solid(local_x as u32, local_y as u32) {
          continue;
        }

        total_samples += 1;

        // Query world pixel at this position
        let Some(pixel) = world.get_pixel(sample_pos) else {
          continue;
        };

        // Check if it's a liquid
        let material = materials.get(pixel.material);
        if material.state == PhysicsState::Liquid {
          liquid_samples += 1;
          liquid_center_sum += Vec2::new(sample_x, sample_y);
        }
      }
    }

    // Calculate submersion fraction and center
    let submerged_fraction = if total_samples > 0 {
      liquid_samples as f32 / total_samples as f32
    } else {
      0.0
    };

    let submerged_center = if liquid_samples > 0 {
      liquid_center_sum / liquid_samples as f32
    } else {
      transform.translation().truncate()
    };

    // Update or insert BuoyancyState
    match state {
      Some(mut s) => {
        s.submerged_fraction = submerged_fraction;
        s.submerged_center = submerged_center;
      }
      None => {
        commands.entity(entity).insert(BuoyancyState {
          submerged_fraction,
          submerged_center,
        });
      }
    }
  }
}
