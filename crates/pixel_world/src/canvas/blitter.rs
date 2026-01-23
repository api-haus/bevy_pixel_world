//! Parallel tile blitter with 2x2 checkerboard scheduling.
//!
//! Tiles are grouped into four phases (A, B, C, D) based on their position
//! modulo 2. Tiles in the same phase are never adjacent, allowing safe parallel
//! execution.

use std::collections::HashMap;
use std::sync::Mutex;

use rayon::prelude::*;

use crate::coords::{ChunkPos, TilePos, WorldFragment, WorldPos, WorldRect, TILE_SIZE};
use crate::pixel::Pixel;
use crate::primitives::Chunk;

/// Phase assignment for 2x2 checkerboard scheduling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
  A, // (0, 1) - top-left of 2x2
  B, // (1, 1) - top-right
  C, // (0, 0) - bottom-left
  D, // (1, 0) - bottom-right
}

impl Phase {
  /// Determine the phase for a tile position.
  pub fn from_tile(tile: TilePos) -> Self {
    let x_mod = tile.0.rem_euclid(2);
    let y_mod = tile.1.rem_euclid(2);
    match (x_mod, y_mod) {
      (0, 1) => Phase::A,
      (1, 1) => Phase::B,
      (0, 0) => Phase::C,
      (1, 0) => Phase::D,
      _ => unreachable!(),
    }
  }

  /// Returns all phases in execution order.
  pub const fn all() -> [Phase; 4] {
    [Phase::A, Phase::B, Phase::C, Phase::D]
  }
}

/// Chunks wrapped in Mutex for safe concurrent access.
pub struct LockedChunks<'a> {
  chunks: HashMap<ChunkPos, Mutex<&'a mut Chunk>>,
}

impl<'a> LockedChunks<'a> {
  /// Creates a new locked chunks wrapper.
  pub fn new(chunks: HashMap<ChunkPos, &'a mut Chunk>) -> Self {
    let locked = chunks
      .into_iter()
      .map(|(pos, chunk)| (pos, Mutex::new(chunk)))
      .collect();
    Self { chunks: locked }
  }

  /// Get a reference to a chunk's mutex, if it exists.
  pub fn get(&self, pos: ChunkPos) -> Option<&Mutex<&'a mut Chunk>> {
    self.chunks.get(&pos)
  }
}

/// Executes a blit operation across tiles in parallel using 2x2 checkerboard
/// scheduling.
pub fn parallel_blit<F>(
  chunks: &LockedChunks<'_>,
  rect: WorldRect,
  f: F,
  dirty_chunks: &Mutex<Vec<ChunkPos>>,
) where
  F: Fn(WorldFragment) -> Option<Pixel> + Sync,
{
  // Group tiles by phase
  let tiles: Vec<TilePos> = rect.to_tile_range().collect();
  let mut phases: [Vec<TilePos>; 4] = [vec![], vec![], vec![], vec![]];

  for tile in tiles {
    let phase = Phase::from_tile(tile);
    let idx = match phase {
      Phase::A => 0,
      Phase::B => 1,
      Phase::C => 2,
      Phase::D => 3,
    };
    phases[idx].push(tile);
  }

  // Precompute UV scaling
  let w_recip = if rect.width > 1 {
    1.0 / (rect.width - 1) as f32
  } else {
    0.0
  };
  let h_recip = if rect.height > 1 {
    1.0 / (rect.height - 1) as f32
  } else {
    0.0
  };

  // Execute each phase sequentially, tiles within phase in parallel
  for phase_tiles in phases {
    phase_tiles.par_iter().for_each(|&tile| {
      process_tile(chunks, tile, &rect, &f, w_recip, h_recip, dirty_chunks);
    });
    // Implicit barrier between phases due to sequential for loop
  }
}

/// Process a single tile, writing pixels that pass the filter.
fn process_tile<F>(
  chunks: &LockedChunks<'_>,
  tile: TilePos,
  rect: &WorldRect,
  f: &F,
  w_recip: f32,
  h_recip: f32,
  dirty_chunks: &Mutex<Vec<ChunkPos>>,
) where
  F: Fn(WorldFragment) -> Option<Pixel> + Sync,
{
  let tile_size = TILE_SIZE as i64;
  let tile_x_start = tile.0 * tile_size;
  let tile_y_start = tile.1 * tile_size;

  // Track which chunks we've dirtied in this tile
  let mut local_dirty: Vec<ChunkPos> = Vec::new();

  for dy in 0..TILE_SIZE {
    let world_y = tile_y_start + dy as i64;

    // Skip if outside rect bounds
    if world_y < rect.y || world_y >= rect.y + rect.height as i64 {
      continue;
    }

    for dx in 0..TILE_SIZE {
      let world_x = tile_x_start + dx as i64;

      // Skip if outside rect bounds
      if world_x < rect.x || world_x >= rect.x + rect.width as i64 {
        continue;
      }

      // Compute normalized coordinates
      let u = (world_x - rect.x) as f32 * w_recip;
      let v = (world_y - rect.y) as f32 * h_recip;

      let frag = WorldFragment {
        x: world_x,
        y: world_y,
        u,
        v,
      };

      // Call the shader function
      if let Some(pixel) = f(frag) {
        // Convert world pos to chunk + local
        let (chunk_pos, local_pos) = WorldPos(world_x, world_y).to_chunk_and_local();

        // Try to write the pixel
        if let Some(chunk_mutex) = chunks.get(chunk_pos) {
          if let Ok(mut chunk) = chunk_mutex.lock() {
            chunk.pixels[(local_pos.0 as u32, local_pos.1 as u32)] = pixel;
            if !local_dirty.contains(&chunk_pos) {
              local_dirty.push(chunk_pos);
            }
          }
        }
      }
    }
  }

  // Merge local dirty list into global
  if !local_dirty.is_empty() {
    if let Ok(mut global_dirty) = dirty_chunks.lock() {
      for pos in local_dirty {
        if !global_dirty.contains(&pos) {
          global_dirty.push(pos);
        }
      }
    }
  }
}
