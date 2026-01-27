//! Submersion sampling for pixel bodies.
//!
//! Samples the world at grid points within a body's AABB to determine
//! how much of the body is submerged in liquid.

use bevy::prelude::*;
use rayon::prelude::*;

use super::{Submergent, SubmersionConfig, SubmersionState};
use crate::coords::{WorldPos, WorldRect};
use crate::material::{Materials, PhysicsState};
use crate::pixel::{Pixel, PixelFlags};
use crate::pixel_body::{PixelBody, compute_world_aabb};
use crate::world::PixelWorld;

/// Result of sampling a body's grid for liquid pixels.
struct GridSampleResult {
  liquid_samples: u32,
  total_samples: u32,
  liquid_center_sum: Vec2,
}

/// Checks if a pixel is liquid (not a body pixel and has liquid physics state).
fn is_liquid_pixel(pixel: &Pixel, materials: &Materials) -> bool {
  // Body pixels can't be liquid for submersion purposes
  if pixel.flags.contains(PixelFlags::PIXEL_BODY) {
    return false;
  }
  let material = materials.get(pixel.material);
  material.state == PhysicsState::Liquid
}

/// Samples a body's AABB grid for liquid pixels.
///
/// For each sample point inside the body, checks adjacent pixels (below and
/// to the sides) for liquid. This handles the case where body pixels have
/// replaced the underlying terrain.
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

      // Check adjacent pixels for liquid (the body pixel itself won't be liquid)
      let sx = sample_x as i64;
      let sy = sample_y as i64;
      let adjacent_offsets = [(0, -1), (0, 1), (-1, 0), (1, 0)]; // below, above, left, right

      let is_adjacent_liquid = adjacent_offsets.iter().any(|(dx, dy)| {
        let pos = WorldPos::new(sx + dx, sy + dy);
        world
          .get_pixel(pos)
          .is_some_and(|p| is_liquid_pixel(p, materials))
      });

      if is_adjacent_liquid {
        result.liquid_samples += 1;
        result.liquid_center_sum += Vec2::new(sample_x, sample_y);
      }
    }
  }

  result
}

/// Computed submersion result for a single body.
struct BodySampleResult {
  entity: Entity,
  has_existing_state: bool,
  submerged_fraction: f32,
  submerged_center: Vec2,
  is_submerged: bool,
  liquid_samples: u32,
  total_samples: u32,
}

/// Samples submersion for all submergent pixel bodies.
///
/// Creates an NxN sample grid across each body's AABB and queries the world
/// for liquid pixels. Updates `SubmersionState` with the fraction submerged,
/// center of buoyancy, and threshold-crossing state.
///
/// Sampling is parallelized across bodies since each body's computation is
/// independent and the world access is read-only.
pub fn sample_submersion(
  mut commands: Commands,
  worlds: Query<&PixelWorld>,
  materials: Res<Materials>,
  config: Res<SubmersionConfig>,
  mut bodies: Query<(
    Entity,
    &PixelBody,
    &GlobalTransform,
    &Submergent,
    Option<&mut SubmersionState>,
  )>,
) {
  let Ok(world) = worlds.single() else {
    return;
  };

  let grid_size = config.sample_grid_size as usize;
  let threshold = config.submersion_threshold;

  // Collect body data for parallel processing
  let body_data: Vec<_> = bodies
    .iter()
    .map(|(entity, body, transform, _, state)| {
      let aabb = compute_world_aabb(body, transform);
      (entity, body, transform, aabb, state.is_some())
    })
    .collect();

  // Parallel sampling phase - read-only world access
  let results: Vec<BodySampleResult> = body_data
    .par_iter()
    .map(|&(entity, body, transform, aabb, has_existing_state)| {
      let result = sample_body_grid(world, &materials, body, transform, aabb, grid_size);

      let submerged_fraction = if result.total_samples > 0 {
        result.liquid_samples as f32 / result.total_samples as f32
      } else {
        0.0
      };

      let submerged_center = if result.liquid_samples > 0 {
        result.liquid_center_sum / result.liquid_samples as f32
      } else {
        transform.translation().truncate()
      };

      BodySampleResult {
        entity,
        has_existing_state,
        submerged_fraction,
        submerged_center,
        is_submerged: submerged_fraction >= threshold,
        liquid_samples: result.liquid_samples,
        total_samples: result.total_samples,
      }
    })
    .collect();

  // Sequential application phase - requires mutable access
  for result in results {
    if result.has_existing_state {
      // Update existing state
      if let Ok((_, _, _, _, Some(mut state))) = bodies.get_mut(result.entity) {
        state.submerged_fraction = result.submerged_fraction;
        state.submerged_center = result.submerged_center;
        state.is_submerged = result.is_submerged;
        state.debug_liquid_samples = result.liquid_samples;
        state.debug_total_samples = result.total_samples;
      }
    } else {
      // Insert new state
      commands.entity(result.entity).insert(SubmersionState {
        is_submerged: result.is_submerged,
        submerged_fraction: result.submerged_fraction,
        submerged_center: result.submerged_center,
        previous_submerged: false,
        debug_liquid_samples: result.liquid_samples,
        debug_total_samples: result.total_samples,
      });
    }
  }
}
