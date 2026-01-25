//! Submersion sampling for buoyancy calculations.
//!
//! Samples the world at grid points within a body's AABB to determine
//! how much of the body is submerged in liquid.

use bevy::prelude::*;

use super::{BuoyancyConfig, BuoyancyState, Buoyant};
use crate::coords::{WorldPos, WorldRect};
use crate::material::{Materials, PhysicsState};
use crate::pixel_body::{PixelBody, compute_world_aabb};
use crate::world::PixelWorld;

/// Result of sampling a body's grid for liquid pixels.
struct GridSampleResult {
  liquid_samples: u32,
  total_samples: u32,
  liquid_center_sum: Vec2,
}

/// Samples a body's AABB grid for liquid pixels.
///
/// Returns counts of liquid and total samples, plus the sum of liquid sample
/// positions.
fn sample_body_grid(
  world: &PixelWorld,
  materials: &Materials,
  body: &PixelBody,
  transform: &GlobalTransform,
  aabb: WorldRect,
  grid_size: usize,
) -> GridSampleResult {
  let step_x = aabb.width as f32 / grid_size as f32;
  let step_y = aabb.height as f32 / grid_size as f32;

  let mut result = GridSampleResult {
    liquid_samples: 0,
    total_samples: 0,
    liquid_center_sum: Vec2::ZERO,
  };

  for gy in 0..grid_size {
    for gx in 0..grid_size {
      let sample_x = aabb.x as f32 + (gx as f32 + 0.5) * step_x;
      let sample_y = aabb.y as f32 + (gy as f32 + 0.5) * step_y;

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

      result.total_samples += 1;

      // Query world pixel at this position
      let sample_pos = WorldPos::new(sample_x as i64, sample_y as i64);
      let Some(pixel) = world.get_pixel(sample_pos) else {
        continue;
      };

      // Check if it's a liquid
      let material = materials.get(pixel.material);
      if material.state == PhysicsState::Liquid {
        result.liquid_samples += 1;
        result.liquid_center_sum += Vec2::new(sample_x, sample_y);
      }
    }
  }

  result
}

/// Computes submersion state from grid sample results.
fn compute_submersion_state(result: &GridSampleResult, fallback_center: Vec2) -> BuoyancyState {
  let submerged_fraction = if result.total_samples > 0 {
    result.liquid_samples as f32 / result.total_samples as f32
  } else {
    0.0
  };

  let submerged_center = if result.liquid_samples > 0 {
    result.liquid_center_sum / result.liquid_samples as f32
  } else {
    fallback_center
  };

  BuoyancyState {
    submerged_fraction,
    submerged_center,
  }
}

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
    let result = sample_body_grid(world, &materials, body, transform, aabb, grid_size);
    let new_state = compute_submersion_state(&result, transform.translation().truncate());

    match state {
      Some(mut s) => {
        s.submerged_fraction = new_state.submerged_fraction;
        s.submerged_center = new_state.submerged_center;
      }
      None => {
        commands.entity(entity).insert(new_state);
      }
    }
  }
}
