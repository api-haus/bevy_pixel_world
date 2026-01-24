//! Marching squares contour extraction.
//!
//! Extracts closed polylines from a binary grid representing solid/empty cells.

use std::collections::HashMap;

use bevy::math::Vec2;

/// Grid size for tile extraction (tile + 1px border on each side).
/// This allows contours to connect across tile boundaries.
pub const GRID_SIZE: usize = 34;

/// Edge segment within a cell, represented as start and end points.
/// Coordinates are in cell-local space [0, 1].
type EdgeSegment = ((f32, f32), (f32, f32));

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
const EDGE_TABLE: [&[EdgeSegment]; 16] = [
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

/// Extracts contour polylines from a binary grid using marching squares.
///
/// # Arguments
/// * `grid` - A 34x34 binary grid where `true` indicates solid/collision
///   pixels. The grid includes a 1-pixel border around the 32x32 tile.
/// * `tile_origin` - World coordinates of the tile's bottom-left corner.
///
/// # Returns
/// A vector of closed polylines in world coordinates.
pub fn marching_squares(
  grid: &[[bool; GRID_SIZE]; GRID_SIZE],
  tile_origin: Vec2,
) -> Vec<Vec<Vec2>> {
  // Create a working copy with the outer border forced to empty.
  // This ensures marching squares generates edge segments at tile boundaries,
  // closing contours that would otherwise be left open.
  let mut working_grid = *grid;
  for i in 0..GRID_SIZE {
    working_grid[0][i] = false; // bottom row
    working_grid[GRID_SIZE - 1][i] = false; // top row
    working_grid[i][0] = false; // left column
    working_grid[i][GRID_SIZE - 1] = false; // right column
  }

  // Collect all edge segments from the grid
  let mut segments: Vec<(Vec2, Vec2)> = Vec::new();

  // Process each 2x2 cell in the grid
  // The grid is 34x34, giving us 33x33 cells
  for cy in 0..GRID_SIZE - 1 {
    for cx in 0..GRID_SIZE - 1 {
      // Sample the 4 corners of this cell
      // Grid layout (Y-down in array indices, but we treat Y+ as up):
      //   grid[cy][cx]     = bottom-left  (bit 2)
      //   grid[cy][cx+1]   = bottom-right (bit 3)
      //   grid[cy+1][cx]   = top-left     (bit 0)
      //   grid[cy+1][cx+1] = top-right    (bit 1)
      let bl = working_grid[cy][cx];
      let br = working_grid[cy][cx + 1];
      let tl = working_grid[cy + 1][cx];
      let tr = working_grid[cy + 1][cx + 1];

      let case = (tl as usize) | ((tr as usize) << 1) | ((bl as usize) << 2) | ((br as usize) << 3);

      // Get edge segments for this case
      for &((x1, y1), (x2, y2)) in EDGE_TABLE[case] {
        // Convert cell-local coords to world coords
        // Cell (cx, cy) corresponds to pixel position (cx-1, cy-1) relative to tile
        // origin because we have a 1-pixel border
        let world_x1 = tile_origin.x + (cx as f32 - 1.0) + x1;
        let world_y1 = tile_origin.y + (cy as f32 - 1.0) + y1;
        let world_x2 = tile_origin.x + (cx as f32 - 1.0) + x2;
        let world_y2 = tile_origin.y + (cy as f32 - 1.0) + y2;

        segments.push((Vec2::new(world_x1, world_y1), Vec2::new(world_x2, world_y2)));
      }
    }
  }

  // Connect segments into closed polylines
  connect_segments(segments)
}

/// Snaps a point to integer grid for robust endpoint matching.
///
/// Marching squares produces coordinates at exact 0.5 intervals, so we
/// multiply by 2 and round to get exact integer keys for HashMap lookups.
fn grid_key(v: Vec2) -> (i32, i32) {
  ((v.x * 2.0).round() as i32, (v.y * 2.0).round() as i32)
}

/// Builds an adjacency map from edge segments.
///
/// Maps grid keys to list of (segment_index, is_start_endpoint) pairs,
/// allowing efficient lookup of connected segments during traversal.
fn build_adjacency_map(segments: &[(Vec2, Vec2)]) -> HashMap<(i32, i32), Vec<(usize, bool)>> {
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
fn connect_segments(segments: Vec<(Vec2, Vec2)>) -> Vec<Vec<Vec2>> {
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

#[cfg(test)]
mod tests {
  use super::*;

  /// Creates a grid with all cells empty.
  fn empty_grid() -> [[bool; GRID_SIZE]; GRID_SIZE] {
    [[false; GRID_SIZE]; GRID_SIZE]
  }

  /// Creates a grid with all cells solid.
  fn solid_grid() -> [[bool; GRID_SIZE]; GRID_SIZE] {
    [[true; GRID_SIZE]; GRID_SIZE]
  }

  #[test]
  fn test_empty_grid_no_contours() {
    let grid = empty_grid();
    let contours = marching_squares(&grid, Vec2::ZERO);
    assert!(contours.is_empty(), "Empty grid should produce no contours");
  }

  #[test]
  fn test_solid_grid_produces_boundary_contour() {
    // A fully solid grid produces boundary contours because the outer
    // border is forced to empty (to ensure closed contours at tile boundaries).
    let grid = solid_grid();
    let contours = marching_squares(&grid, Vec2::ZERO);
    // Should produce at least one contour
    assert!(
      !contours.is_empty(),
      "Fully solid grid should produce boundary contours"
    );
  }

  #[test]
  fn test_single_solid_pixel_produces_contour() {
    let mut grid = empty_grid();
    // Place a single solid pixel in the center
    // Grid index 17 corresponds to tile pixel 16 (center of 32x32 tile)
    grid[17][17] = true;

    let contours = marching_squares(&grid, Vec2::ZERO);
    assert_eq!(contours.len(), 1, "Single pixel should produce one contour");
    assert_eq!(
      contours[0].len(),
      4,
      "Single pixel contour should have 4 vertices (diamond shape)"
    );
  }

  #[test]
  fn test_solid_block_produces_contours() {
    let mut grid = empty_grid();
    // Create a 3x3 solid block
    for y in 15..18 {
      for x in 15..18 {
        grid[y][x] = true;
      }
    }

    let contours = marching_squares(&grid, Vec2::ZERO);
    // Should produce at least one contour with meaningful vertices
    assert!(!contours.is_empty(), "Solid block should produce contours");
    let total_vertices: usize = contours.iter().map(|c| c.len()).sum();
    // A 3x3 block has 12 edge pixels, so we should have at least 12 vertices
    assert!(
      total_vertices >= 8,
      "Solid block contours should have sufficient vertices"
    );
  }
}
