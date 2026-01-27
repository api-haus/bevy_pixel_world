//! Reusable grid sampling for pixel bodies.
//!
//! Provides a predicate-parameterized grid sampler that queries the pixel world
//! at evenly spaced points within a body's AABB.

use bevy::prelude::*;

use crate::coords::{WorldPos, WorldRect};
use crate::material::Materials;
use crate::pixel::Pixel;
use crate::pixel_body::PixelBody;
use crate::world::PixelWorld;

/// Result of sampling a body's grid with a predicate.
pub struct GridSampleResult {
  /// Number of sample points where the predicate matched.
  pub matched_samples: u32,
  /// Total number of sample points that hit solid body pixels.
  pub total_samples: u32,
  /// Sum of world positions of matched samples (divide by `matched_samples`
  /// to get center).
  pub matched_center_sum: Vec2,
}

/// Configuration for grid sampling.
#[derive(Resource, Clone, Debug)]
pub struct GridSampleConfig {
  /// Size of the sample grid (NxN samples across body AABB).
  /// Higher values are more accurate but slower. Default: 4.
  pub sample_grid_size: u8,
}

impl Default for GridSampleConfig {
  fn default() -> Self {
    Self {
      sample_grid_size: 4,
    }
  }
}

/// Samples a body's AABB grid with the given predicate.
///
/// For each sample point inside the body, checks adjacent pixels (below, above,
/// left, right) with the predicate. This handles the case where body pixels
/// have replaced the underlying terrain.
pub fn sample_body_grid(
  world: &PixelWorld,
  materials: &Materials,
  body: &PixelBody,
  transform: &GlobalTransform,
  aabb: WorldRect,
  grid_size: usize,
  predicate: impl Fn(&Pixel, &Materials) -> bool,
) -> GridSampleResult {
  let step_x = aabb.width as f32 / grid_size as f32;
  let step_y = aabb.height as f32 / grid_size as f32;

  let mut result = GridSampleResult {
    matched_samples: 0,
    total_samples: 0,
    matched_center_sum: Vec2::ZERO,
  };

  let inverse = transform.affine().inverse();

  for gy in 0..grid_size {
    for gx in 0..grid_size {
      let sample_x = aabb.x as f32 + (gx as f32 + 0.5) * step_x;
      let sample_y = aabb.y as f32 + (gy as f32 + 0.5) * step_y;

      let world_point = Vec3::new(sample_x, sample_y, 0.0);
      if body.world_to_solid_local(world_point, &inverse).is_none() {
        continue;
      }

      result.total_samples += 1;

      // Check adjacent pixels (the body pixel itself won't match for overlap queries)
      let sx = sample_x as i64;
      let sy = sample_y as i64;
      let adjacent_offsets = [(0, -1), (0, 1), (-1, 0), (1, 0)];

      let is_adjacent_match = adjacent_offsets.iter().any(|(dx, dy)| {
        let pos = WorldPos::new(sx + dx, sy + dy);
        world
          .get_pixel(pos)
          .is_some_and(|p| predicate(p, materials))
      });

      if is_adjacent_match {
        result.matched_samples += 1;
        result.matched_center_sum += Vec2::new(sample_x, sample_y);
      }
    }
  }

  result
}
