//! Collider generation from pixel body shape masks.
//!
//! Uses marching squares to extract contours from the shape mask, then
//! triangulates for physics collision.

#[cfg(feature = "avian2d")]
use avian2d::prelude::Collider;
#[cfg(physics)]
use bevy::math::Vec2;
#[cfg(feature = "rapier2d")]
use bevy_rapier2d::prelude::Collider;

use super::PixelBody;
#[cfg(physics)]
use crate::collision::{
  connect_segments, extract_marching_segments, simplify_polylines, triangulate_polygons,
};

/// Generates a physics collider from a pixel body's shape mask.
///
/// Uses marching squares to extract contours, Douglas-Peucker simplification
/// to reduce vertex count, and triangulation for physics collision.
///
/// Returns None if the shape mask is empty or produces no valid geometry.
#[cfg(physics)]
pub fn generate_collider(body: &PixelBody) -> Option<Collider> {
  generate_collider_with_tolerance(body, 0.5)
}

/// Generates a collider with custom simplification tolerance.
#[cfg(physics)]
pub fn generate_collider_with_tolerance(body: &PixelBody, tolerance: f32) -> Option<Collider> {
  let width = body.width();
  let height = body.height();

  if body.is_empty() {
    return None;
  }

  // Build a grid for marching squares
  // We need GRID_SIZE (34) but our body may be smaller or larger
  // Pad to ensure we have a 1-pixel empty border
  let grid_width = width as usize + 2;
  let grid_height = height as usize + 2;

  // For simplicity, use a dynamic grid size approach
  let grid = build_marching_grid(body);

  // Extract contours - use origin (0, 0) since we want local coordinates
  let polylines = extract_contours(&grid, grid_width, grid_height);

  if polylines.is_empty() {
    return None;
  }

  // Simplify polylines
  let simplified = simplify_polylines(polylines, tolerance);

  if simplified.is_empty() {
    return None;
  }

  // Offset polylines to be centered around origin
  let offset = Vec2::new(body.origin.x as f32, body.origin.y as f32);
  let offset_polylines: Vec<Vec<Vec2>> = simplified
    .into_iter()
    .map(|poly| poly.into_iter().map(|v| v + offset).collect())
    .collect();

  // Triangulate and build compound collider
  let triangulated = triangulate_polygons(&offset_polylines);

  if triangulated.is_empty() {
    return None;
  }

  // Build compound collider from triangles
  let shapes: Vec<(Vec2, f32, Collider)> = triangulated
    .iter()
    .flat_map(|(vertices, triangles)| {
      triangles.iter().filter_map(|tri| {
        let a = vertices[tri.a];
        let b = vertices[tri.b];
        let c = vertices[tri.c];
        // Skip degenerate triangles that crash parry2d's BVH
        let cross = (b - a).perp_dot(c - a);
        if cross.abs() > f32::EPSILON {
          Some((Vec2::ZERO, 0.0, Collider::triangle(a, b, c)))
        } else {
          None
        }
      })
    })
    .collect();

  if shapes.is_empty() {
    return None;
  }

  Some(Collider::compound(shapes))
}

/// Builds a boolean grid from the shape mask for marching squares.
#[cfg(physics)]
fn build_marching_grid(body: &PixelBody) -> Vec<Vec<bool>> {
  let width = body.width() as usize;
  let height = body.height() as usize;

  // Add 1-pixel border on each side
  let grid_width = width + 2;
  let grid_height = height + 2;

  let mut grid = vec![vec![false; grid_width]; grid_height];

  for y in 0..height {
    for x in 0..width {
      if body.is_solid(x as u32, y as u32) {
        // Offset by 1 for the border
        grid[y + 1][x + 1] = true;
      }
    }
  }

  grid
}

/// Extracts contour polylines from a dynamic-sized grid.
///
/// Similar to marching_squares but works with arbitrary grid sizes.
#[cfg(physics)]
fn extract_contours(grid: &[Vec<bool>], width: usize, height: usize) -> Vec<Vec<Vec2>> {
  let segments = extract_marching_segments(width, height, |x, y| grid[y][x], 1.0);
  connect_segments(segments)
}

/// Stub for when no physics feature is enabled.
#[cfg(not(physics))]
pub fn generate_collider(_body: &PixelBody) -> Option<()> {
  None
}
