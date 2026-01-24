//! Shared contour utilities for marching squares.
//!
//! Provides segment connection algorithms used by both tile-based collision
//! extraction and pixel body collider generation.

use std::collections::HashMap;

use bevy::math::Vec2;

/// Edge segment within a cell, represented as start and end points.
/// Coordinates are in cell-local space [0, 1].
pub type EdgeSegment = ((f32, f32), (f32, f32));

/// Lookup table for marching squares edge segments.
///
/// Each cell has 4 corners sampled with bit positions:
///   - bit 0 (1): top-left (tl)
///   - bit 1 (2): top-right (tr)
///   - bit 2 (4): bottom-left (bl)
///   - bit 3 (8): bottom-right (br)
///
/// Case index is computed as: tl | (tr << 1) | (bl << 2) | (br << 3)
///
/// Cell coordinate system (Y+ up):
///   (0,1)----(0.5,1)----(1,1)
///     |                   |
///   (0,0.5)            (1,0.5)
///     |                   |
///   (0,0)----(0.5,0)----(1,0)
///
/// Returns 0, 1, or 2 edge segments per cell case.
pub const EDGE_TABLE: [&[EdgeSegment]; 16] = [
  // Case 0 (0000): all empty - no contour
  &[],
  // Case 1 (0001): tl solid - diagonal from left to top
  &[((0.0, 0.5), (0.5, 1.0))],
  // Case 2 (0010): tr solid - diagonal from top to right
  &[((0.5, 1.0), (1.0, 0.5))],
  // Case 3 (0011): tl+tr solid (top row) - horizontal from left to right
  &[((0.0, 0.5), (1.0, 0.5))],
  // Case 4 (0100): bl solid - diagonal from bottom to left
  &[((0.5, 0.0), (0.0, 0.5))],
  // Case 5 (0101): tl+bl solid (left column) - vertical from bottom to top
  &[((0.5, 0.0), (0.5, 1.0))],
  // Case 6 (0110): tr+bl solid (diagonal saddle) - two separate contours
  &[((0.0, 0.5), (0.5, 1.0)), ((0.5, 0.0), (1.0, 0.5))],
  // Case 7 (0111): tl+tr+bl solid (only br empty) - diagonal from bottom to right
  &[((0.5, 0.0), (1.0, 0.5))],
  // Case 8 (1000): br solid - diagonal from right to bottom
  &[((1.0, 0.5), (0.5, 0.0))],
  // Case 9 (1001): tl+br solid (diagonal saddle) - two separate contours
  &[((0.0, 0.5), (0.5, 0.0)), ((0.5, 1.0), (1.0, 0.5))],
  // Case 10 (1010): tr+br solid (right column) - vertical from top to bottom
  &[((0.5, 1.0), (0.5, 0.0))],
  // Case 11 (1011): tl+tr+br solid (only bl empty) - diagonal from left to bottom
  &[((0.0, 0.5), (0.5, 0.0))],
  // Case 12 (1100): bl+br solid (bottom row) - horizontal from right to left
  &[((1.0, 0.5), (0.0, 0.5))],
  // Case 13 (1101): tl+bl+br solid (only tr empty) - diagonal from top to right
  &[((0.5, 1.0), (1.0, 0.5))],
  // Case 14 (1110): tr+bl+br solid (only tl empty) - diagonal from left to top
  &[((0.0, 0.5), (0.5, 1.0))],
  // Case 15 (1111): all solid - no contour
  &[],
];

/// Snaps a point to integer grid for robust endpoint matching.
///
/// Marching squares produces coordinates at exact 0.5 intervals, so we
/// multiply by 2 and round to get exact integer keys for HashMap lookups.
pub fn grid_key(v: Vec2) -> (i32, i32) {
  ((v.x * 2.0).round() as i32, (v.y * 2.0).round() as i32)
}

/// Builds an adjacency map from edge segments.
///
/// Maps grid keys to list of (segment_index, is_start_endpoint) pairs,
/// allowing efficient lookup of connected segments during traversal.
pub fn build_adjacency_map(segments: &[(Vec2, Vec2)]) -> HashMap<(i32, i32), Vec<(usize, bool)>> {
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
  adjacency
}

/// Logs endpoints that have only one connected segment (debug builds only).
#[cfg(debug_assertions)]
fn log_solo_endpoints(adjacency: &HashMap<(i32, i32), Vec<(usize, bool)>>, segment_count: usize) {
  let solo_keys: Vec<_> = adjacency
    .iter()
    .filter(|(_, entries)| entries.len() == 1)
    .take(5)
    .map(|(key, _)| *key)
    .collect();

  if !solo_keys.is_empty() {
    let solo_coords: Vec<_> = solo_keys
      .iter()
      .map(|(x, y)| format!("({}, {})", *x as f32 / 2.0, *y as f32 / 2.0))
      .collect();
    bevy::log::debug!(
      "connect_segments: {} segments, {} solo endpoints at {:?}",
      segment_count,
      adjacency.values().filter(|e| e.len() == 1).count(),
      solo_coords
    );
  }
}

/// Traverses connected segments starting from a given index, building a
/// polyline.
///
/// Returns the vertices of the polyline in traversal order, with duplicate
/// closing vertex removed if the polyline forms a closed loop.
fn traverse_polyline(
  segments: &[(Vec2, Vec2)],
  adjacency: &HashMap<(i32, i32), Vec<(usize, bool)>>,
  used: &mut [bool],
  start_idx: usize,
) -> Vec<Vec2> {
  let mut polyline = Vec::new();
  let mut current_idx = start_idx;
  let mut entering_from_start = true;

  loop {
    used[current_idx] = true;
    let (seg_start, seg_end) = segments[current_idx];

    // Add vertices in traversal order
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

    // Find next segment sharing current endpoint
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

  // Remove duplicate closing vertex if the polyline forms a closed loop
  if polyline.len() >= 4 {
    let first = polyline.first().unwrap();
    let last = polyline.last().unwrap();
    if (first.x - last.x).abs() < 0.001 && (first.y - last.y).abs() < 0.001 {
      polyline.pop();
    }
  }

  polyline
}

/// Connects edge segments into closed polylines.
///
/// Uses integer grid-based matching for robust endpoint comparison.
pub fn connect_segments(segments: Vec<(Vec2, Vec2)>) -> Vec<Vec<Vec2>> {
  if segments.is_empty() {
    return vec![];
  }

  let adjacency = build_adjacency_map(&segments);

  #[cfg(debug_assertions)]
  log_solo_endpoints(&adjacency, segments.len());

  let mut used = vec![false; segments.len()];
  let mut polylines = Vec::new();

  for start_idx in 0..segments.len() {
    if used[start_idx] {
      continue;
    }

    let polyline = traverse_polyline(&segments, &adjacency, &mut used, start_idx);
    if polyline.len() >= 3 {
      polylines.push(polyline);
    }
  }

  polylines
}
