//! Collider generation from pixel body shape masks.
//!
//! Uses marching squares to extract contours from the shape mask, then
//! triangulates for physics collision.

#[cfg(feature = "avian2d")]
use avian2d::prelude::Collider;
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
use bevy::math::Vec2;
#[cfg(feature = "rapier2d")]
use bevy_rapier2d::prelude::Collider;

use super::PixelBody;
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
use crate::collision::{simplify_polylines, triangulate_polygons};

/// Generates a physics collider from a pixel body's shape mask.
///
/// Uses marching squares to extract contours, Douglas-Peucker simplification
/// to reduce vertex count, and triangulation for physics collision.
///
/// Returns None if the shape mask is empty or produces no valid geometry.
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
pub fn generate_collider(body: &PixelBody) -> Option<Collider> {
  generate_collider_with_tolerance(body, 0.5)
}

/// Generates a collider with custom simplification tolerance.
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
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
        Some((Vec2::ZERO, 0.0, Collider::triangle(a, b, c)))
      })
    })
    .collect();

  if shapes.is_empty() {
    return None;
  }

  Some(Collider::compound(shapes))
}

/// Builds a boolean grid from the shape mask for marching squares.
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
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
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
fn extract_contours(grid: &[Vec<bool>], width: usize, height: usize) -> Vec<Vec<Vec2>> {
  // Edge segment lookup table (same as in marching.rs)
  type EdgeSegment = ((f32, f32), (f32, f32));
  const EDGE_TABLE: [&[EdgeSegment]; 16] = [
    &[],
    &[((0.0, 0.5), (0.5, 1.0))],
    &[((0.5, 1.0), (1.0, 0.5))],
    &[((0.0, 0.5), (1.0, 0.5))],
    &[((0.5, 0.0), (0.0, 0.5))],
    &[((0.5, 0.0), (0.5, 1.0))],
    &[((0.0, 0.5), (0.5, 1.0)), ((0.5, 0.0), (1.0, 0.5))],
    &[((0.5, 0.0), (1.0, 0.5))],
    &[((1.0, 0.5), (0.5, 0.0))],
    &[((0.0, 0.5), (0.5, 0.0)), ((0.5, 1.0), (1.0, 0.5))],
    &[((0.5, 1.0), (0.5, 0.0))],
    &[((0.0, 0.5), (0.5, 0.0))],
    &[((1.0, 0.5), (0.0, 0.5))],
    &[((0.5, 1.0), (1.0, 0.5))],
    &[((0.0, 0.5), (0.5, 1.0))],
    &[],
  ];

  let mut segments: Vec<(Vec2, Vec2)> = Vec::new();

  // Process each 2x2 cell
  for cy in 0..height - 1 {
    for cx in 0..width - 1 {
      let bl = grid[cy][cx];
      let br = grid[cy][cx + 1];
      let tl = grid[cy + 1][cx];
      let tr = grid[cy + 1][cx + 1];

      let case = (tl as usize) | ((tr as usize) << 1) | ((bl as usize) << 2) | ((br as usize) << 3);

      for &((x1, y1), (x2, y2)) in EDGE_TABLE[case] {
        // Convert to local coordinates (subtract 1 for border offset)
        let local_x1 = (cx as f32 - 1.0) + x1;
        let local_y1 = (cy as f32 - 1.0) + y1;
        let local_x2 = (cx as f32 - 1.0) + x2;
        let local_y2 = (cy as f32 - 1.0) + y2;

        segments.push((Vec2::new(local_x1, local_y1), Vec2::new(local_x2, local_y2)));
      }
    }
  }

  connect_segments(segments)
}

/// Connects edge segments into closed polylines.
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
fn connect_segments(segments: Vec<(Vec2, Vec2)>) -> Vec<Vec<Vec2>> {
  use std::collections::HashMap;

  if segments.is_empty() {
    return vec![];
  }

  fn grid_key(v: Vec2) -> (i32, i32) {
    ((v.x * 2.0).round() as i32, (v.y * 2.0).round() as i32)
  }

  // Build adjacency map
  let mut adjacency: HashMap<(i32, i32), Vec<(usize, bool)>> = HashMap::new();
  for (i, (start, end)) in segments.iter().enumerate() {
    adjacency
      .entry(grid_key(*start))
      .or_default()
      .push((i, true));
    adjacency
      .entry(grid_key(*end))
      .or_default()
      .push((i, false));
  }

  let mut used = vec![false; segments.len()];
  let mut polylines = Vec::new();

  for start_idx in 0..segments.len() {
    if used[start_idx] {
      continue;
    }

    let mut polyline = Vec::new();
    let mut current_idx = start_idx;
    let mut entering_from_start = true;

    loop {
      used[current_idx] = true;
      let (seg_start, seg_end) = segments[current_idx];

      if entering_from_start {
        if polyline.is_empty() {
          polyline.push(seg_start);
        }
        polyline.push(seg_end);
      } else {
        if polyline.is_empty() {
          polyline.push(seg_end);
        }
        polyline.push(seg_start);
      }

      let current_end = *polyline.last().unwrap();
      let key = grid_key(current_end);

      let next = adjacency
        .get(&key)
        .and_then(|neighbors| neighbors.iter().find(|(idx, _)| !used[*idx]).copied());

      match next {
        Some((idx, is_start)) => {
          current_idx = idx;
          entering_from_start = is_start;
        }
        None => break,
      }
    }

    // Remove duplicate closing vertex
    if polyline.len() >= 4 {
      let first = polyline.first().unwrap();
      let last = polyline.last().unwrap();
      if (first.x - last.x).abs() < 0.001 && (first.y - last.y).abs() < 0.001 {
        polyline.pop();
      }
    }

    if polyline.len() >= 3 {
      polylines.push(polyline);
    }
  }

  polylines
}

/// Stub for when no physics feature is enabled.
#[cfg(not(any(feature = "avian2d", feature = "rapier2d")))]
pub fn generate_collider(_body: &PixelBody) -> Option<()> {
  None
}
