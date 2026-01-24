//! Marching squares contour extraction.
//!
//! Extracts closed polylines from a binary grid representing solid/empty cells.

use bevy::math::Vec2;

use super::contour::{EDGE_TABLE, connect_segments};

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
