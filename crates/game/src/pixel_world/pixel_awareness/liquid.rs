//! Liquid fraction query for pixel bodies.
//!
//! Samples the world around each body to determine what fraction of the body
//! is adjacent to liquid pixels.

use bevy::prelude::*;
use rayon::prelude::*;

use super::grid_sampler::{GridSampleConfig, sample_body_grid};
use crate::pixel_world::material::{Materials, PhysicsState};
use crate::pixel_world::pixel::{Pixel, PixelFlags};
use crate::pixel_world::pixel_body::{PixelBody, compute_world_aabb};
use crate::pixel_world::world::PixelWorld;

/// Tracks liquid adjacency for a pixel body.
///
/// Automatically added to entities with a [`PixelBody`] when they're first
/// sampled. Contains information about how much of the body is adjacent to
/// liquid pixels.
#[derive(Component, Default)]
pub struct LiquidFractionState {
  /// Fraction of the body adjacent to liquid (0.0 to 1.0).
  pub liquid_fraction: f32,
  /// World position of the center of liquid-adjacent samples.
  pub liquid_center: Vec2,
  /// Debug: number of sample points that hit liquid.
  pub debug_liquid_samples: u32,
  /// Debug: total number of sample points that hit solid body pixels.
  pub debug_total_samples: u32,
}

/// Checks if a pixel is liquid (not a body pixel and has liquid physics state).
fn is_liquid_pixel(pixel: &Pixel, materials: &Materials) -> bool {
  if pixel.flags.contains(PixelFlags::PIXEL_BODY) {
    return false;
  }
  let material = materials.get(pixel.material);
  material.state == PhysicsState::Liquid
}

/// Computed liquid fraction result for a single body.
struct BodyLiquidResult {
  entity: Entity,
  has_existing_state: bool,
  liquid_fraction: f32,
  liquid_center: Vec2,
  liquid_samples: u32,
  total_samples: u32,
}

/// Samples liquid fraction for all pixel bodies.
///
/// Creates an NxN sample grid across each body's AABB and queries the world
/// for liquid pixels. Updates [`LiquidFractionState`] with the fraction,
/// center, and debug statistics.
///
/// Sampling is parallelized across bodies since each body's computation is
/// independent and the world access is read-only.
pub fn sample_liquid_fraction(
  mut commands: Commands,
  worlds: Query<&PixelWorld>,
  materials: Res<Materials>,
  config: Res<GridSampleConfig>,
  mut bodies: Query<(
    Entity,
    &PixelBody,
    &GlobalTransform,
    Option<&mut LiquidFractionState>,
  )>,
) {
  let Ok(world) = worlds.single() else {
    return;
  };

  let grid_size = config.sample_grid_size as usize;

  // Collect body data for parallel processing
  let body_data: Vec<_> = bodies
    .iter()
    .map(|(entity, body, transform, state)| {
      let aabb = compute_world_aabb(body, transform);
      (entity, body, transform, aabb, state.is_some())
    })
    .collect();

  // Parallel sampling phase - read-only world access
  let results: Vec<BodyLiquidResult> = body_data
    .par_iter()
    .map(|&(entity, body, transform, aabb, has_existing_state)| {
      let result = sample_body_grid(
        world,
        &materials,
        body,
        transform,
        aabb,
        grid_size,
        is_liquid_pixel,
      );

      let liquid_fraction = if result.total_samples > 0 {
        result.matched_samples as f32 / result.total_samples as f32
      } else {
        0.0
      };

      let liquid_center = if result.matched_samples > 0 {
        result.matched_center_sum / result.matched_samples as f32
      } else {
        transform.translation().truncate()
      };

      BodyLiquidResult {
        entity,
        has_existing_state,
        liquid_fraction,
        liquid_center,
        liquid_samples: result.matched_samples,
        total_samples: result.total_samples,
      }
    })
    .collect();

  // Sequential application phase - requires mutable access
  for result in results {
    if result.has_existing_state {
      if let Ok((_, _, _, Some(mut state))) = bodies.get_mut(result.entity) {
        state.liquid_fraction = result.liquid_fraction;
        state.liquid_center = result.liquid_center;
        state.debug_liquid_samples = result.liquid_samples;
        state.debug_total_samples = result.total_samples;
      }
    } else {
      commands.entity(result.entity).insert(LiquidFractionState {
        liquid_fraction: result.liquid_fraction,
        liquid_center: result.liquid_center,
        debug_liquid_samples: result.liquid_samples,
        debug_total_samples: result.total_samples,
      });
    }
  }
}
