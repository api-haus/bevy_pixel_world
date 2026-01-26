//! 2x2 checkerboard scheduling helpers.
//!
//! This module provides utilities for organizing tiles into four phases
//! based on their position modulo 2. Tiles in the same phase are never
//! adjacent, enabling safe parallel processing.

use super::canvas::Canvas;
use crate::coords::{LocalPos, TILE_SIZE, TILES_PER_CHUNK, TilePos, WorldPos};
use crate::primitives::TileBounds;

/// Neighbor positions that should be woken when a pixel moves.
///
/// When a pixel vacates a position, neighbors above and to the sides
/// may now be able to fall into the empty space.
pub(super) const WAKE_NEIGHBORS: [(i64, i64); 5] = [
  (0, 1),  // above
  (-1, 1), // above-left
  (1, 1),  // above-right
  (-1, 0), // left
  (1, 0),  // right
];

/// Returns tile offsets that overlap with a jittered tile.
///
/// A jittered tile at (tx, ty) with jitter (jx, jy) overlaps:
/// - Original tile (tx, ty) - always
/// - Original tile (tx+1, ty) - if jx > 0
/// - Original tile (tx, ty+1) - if jy > 0
/// - Original tile (tx+1, ty+1) - if jx > 0 and jy > 0
pub(super) fn overlapping_tile_offsets(jitter: (i64, i64)) -> impl Iterator<Item = (i64, i64)> {
  let (jx, jy) = jitter;
  std::iter::once((0i64, 0i64))
    .chain((jx > 0).then_some((1, 0)))
    .chain((jy > 0).then_some((0, 1)))
    .chain((jx > 0 && jy > 0).then_some((1, 1)))
}

/// Transforms world bounds to local tile coordinates.
///
/// Returns None if the transformed bounds are empty (no overlap with tile).
pub(super) fn transform_bounds_to_local(
  world_bounds: (i64, i64, i64, i64),
  jittered_base: (i64, i64),
) -> Option<(u8, u8, u8, u8)> {
  let (world_min_x, world_min_y, world_max_x, world_max_y) = world_bounds;
  let (base_x, base_y) = jittered_base;

  // Convert to jittered tile local coords and clamp to [0, 31]
  let local_min_x = (world_min_x - base_x).clamp(0, 31) as u8;
  let local_min_y = (world_min_y - base_y).clamp(0, 31) as u8;
  let local_max_x = (world_max_x - base_x).clamp(0, 31) as u8;
  let local_max_y = (world_max_y - base_y).clamp(0, 31) as u8;

  // Empty rect check (can happen when dirty region doesn't overlap jittered tile)
  if local_min_x > local_max_x || local_min_y > local_max_y {
    return None;
  }

  Some((local_min_x, local_min_y, local_max_x, local_max_y))
}

/// Unions two optional bounds, returning the combined bounding box.
pub(super) fn union_bounds(a: Option<(u8, u8, u8, u8)>, b: (u8, u8, u8, u8)) -> (u8, u8, u8, u8) {
  match a {
    None => b,
    Some((a_min_x, a_min_y, a_max_x, a_max_y)) => (
      a_min_x.min(b.0),
      a_min_y.min(b.1),
      a_max_x.max(b.2),
      a_max_y.max(b.3),
    ),
  }
}

/// Compute the union of dirty bounds from all original tiles that overlap
/// a jittered tile.
pub(super) fn union_dirty_bounds(
  chunks: &Canvas<'_>,
  tile: TilePos,
  jitter: (i64, i64),
) -> Option<(u8, u8, u8, u8)> {
  let tile_size = TILE_SIZE as i64;

  // Jittered tile base position
  let jittered_base = (tile.x * tile_size + jitter.0, tile.y * tile_size + jitter.1);

  let mut result: Option<(u8, u8, u8, u8)> = None;

  for (dx, dy) in overlapping_tile_offsets(jitter) {
    let orig_base_x = (tile.x + dx) * tile_size;
    let orig_base_y = (tile.y + dy) * tile_size;

    // Get chunk and tile-local coordinates for this original tile
    let (chunk_pos, local_pos) = WorldPos::new(orig_base_x, orig_base_y).to_chunk_and_local();
    let tx = (local_pos.x as u32) / TILE_SIZE;
    let ty = (local_pos.y as u32) / TILE_SIZE;

    let Some(chunk) = chunks.get(chunk_pos) else {
      continue;
    };

    let Some(TileBounds {
      min_x,
      min_y,
      max_x,
      max_y,
    }) = chunk.tile_dirty_rect(tx, ty).bounds()
    else {
      continue;
    };

    // Convert original tile dirty bounds to world coords
    let world_bounds = (
      orig_base_x + min_x as i64,
      orig_base_y + min_y as i64,
      orig_base_x + max_x as i64,
      orig_base_y + max_y as i64,
    );

    if let Some(local_bounds) = transform_bounds_to_local(world_bounds, jittered_base) {
      result = Some(union_bounds(result, local_bounds));
    }
  }

  result
}

/// Returns an iterator over adjacent tiles that need collision updates.
///
/// When a pixel at the boundary of a tile changes, adjacent tiles also need
/// their collision meshes updated (since they sample a 1px border).
///
/// Yields (tile_x, tile_y) pairs for valid adjacent tiles within chunk bounds.
pub(super) fn adjacent_tiles_at_boundary(
  px: u32,
  py: u32,
  tx: u32,
  ty: u32,
) -> impl Iterator<Item = (u32, u32)> {
  let max_local = TILE_SIZE - 1;
  let tiles_per_chunk = TILES_PER_CHUNK;

  let at_left = px == 0;
  let at_right = px == max_local;
  let at_bottom = py == 0;
  let at_top = py == max_local;

  // Compute offsets for adjacent tiles based on boundary position
  // Each offset is (dx, dy) where -1/0/+1 indicates direction
  let offsets: [(i32, i32); 8] = [
    (-1, 0),  // left
    (1, 0),   // right
    (0, -1),  // bottom
    (0, 1),   // top
    (-1, -1), // bottom-left
    (1, -1),  // bottom-right
    (-1, 1),  // top-left
    (1, 1),   // top-right
  ];

  let conditions: [bool; 8] = [
    at_left,
    at_right,
    at_bottom,
    at_top,
    at_left && at_bottom,
    at_right && at_bottom,
    at_left && at_top,
    at_right && at_top,
  ];

  offsets
    .into_iter()
    .zip(conditions)
    .filter_map(move |((dx, dy), should_check)| {
      if !should_check {
        return None;
      }
      let nx = tx as i32 + dx;
      let ny = ty as i32 + dy;
      if nx >= 0 && (nx as u32) < tiles_per_chunk && ny >= 0 && (ny as u32) < tiles_per_chunk {
        Some((nx as u32, ny as u32))
      } else {
        None
      }
    })
}

/// Mark pixels as dirty for simulation in the next pass.
pub(super) fn mark_pixels_dirty(
  chunks: &Canvas<'_>,
  dirty_pixels: &[(crate::coords::ChunkPos, LocalPos)],
) {
  for &(chunk_pos, local) in dirty_pixels {
    if let Some(chunk) = chunks.get_mut(chunk_pos) {
      chunk.mark_pixel_dirty(local.x as u32, local.y as u32);
    }
  }
}

/// Ticks the dirty rect for the owned original tile.
///
/// This maintains the dirty rect state machine correctly: reset before
/// processing, then grow during simulation.
pub(super) fn tick_owned_tile(chunks: &Canvas<'_>, tile: TilePos) {
  let tile_size = TILE_SIZE as i64;
  let orig_base_x = tile.x * tile_size;
  let orig_base_y = tile.y * tile_size;
  let (chunk_pos, local_pos) = WorldPos::new(orig_base_x, orig_base_y).to_chunk_and_local();
  let tx = (local_pos.x as u32) / TILE_SIZE;
  let ty = (local_pos.y as u32) / TILE_SIZE;

  if let Some(chunk) = chunks.get_mut(chunk_pos) {
    chunk.tile_dirty_rect_mut(tx, ty).tick();
  }
}
