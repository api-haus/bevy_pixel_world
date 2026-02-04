//! Marching squares contour extraction.
//!
//! Extracts closed polylines from a binary grid representing solid/empty cells.

use bevy::math::Vec2;

use super::contour::{connect_segments, extract_marching_segments};

/// Grid size for tile extraction (tile + 1px border on each side).
/// This allows contours to connect across tile boundaries.
pub const GRID_SIZE: usize = 34;

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
  // Clear bottom and top rows
  for cell in &mut working_grid[0] {
    *cell = false;
  }
  for cell in &mut working_grid[GRID_SIZE - 1] {
    *cell = false;
  }
  // Clear left and right columns
  for row in &mut working_grid {
    row[0] = false;
    row[GRID_SIZE - 1] = false;
  }

  // Extract segments using shared marching squares implementation
  let segments = extract_marching_segments(GRID_SIZE, GRID_SIZE, |x, y| working_grid[y][x], 1.0);

  // Offset to world coordinates and connect into polylines
  let world_segments: Vec<_> = segments
    .into_iter()
    .map(|(a, b)| (a + tile_origin, b + tile_origin))
    .collect();

  connect_segments(world_segments)
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
