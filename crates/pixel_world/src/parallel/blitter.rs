//! Parallel tile blitter with 2x2 checkerboard scheduling.
//!
//! Tiles are grouped into four phases (A, B, C, D) based on their position
//! modulo 2. Tiles in the same phase are never adjacent, allowing safe parallel
//! execution.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use rayon::prelude::*;

use crate::coords::{ChunkPos, LocalPos, TILE_SIZE, TilePos, WorldFragment, WorldPos, WorldRect};
use crate::debug_shim::{self, DebugGizmos};
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
  fn from_tile(tile: TilePos) -> Self {
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
}

/// Direct chunk access without locks.
///
/// # Safety
/// This type provides interior mutability without runtime checks.
/// It is only safe to use with the 2x2 checkerboard scheduling, which
/// guarantees tiles in the same phase never access overlapping pixels.
pub struct ChunkAccess<'a> {
  chunks: HashMap<ChunkPos, *mut Chunk>,
  _marker: std::marker::PhantomData<&'a mut Chunk>,
}

// SAFETY: The 2x2 checkerboard scheduling guarantees that tiles processed
// in parallel never access overlapping pixel regions.
unsafe impl Send for ChunkAccess<'_> {}
unsafe impl Sync for ChunkAccess<'_> {}

impl<'a> ChunkAccess<'a> {
  /// Creates a new chunk access wrapper from mutable chunk references.
  pub fn new(chunks: HashMap<ChunkPos, &'a mut Chunk>) -> Self {
    let ptrs = chunks
      .into_iter()
      .map(|(pos, chunk)| (pos, chunk as *mut Chunk))
      .collect();
    Self {
      chunks: ptrs,
      _marker: std::marker::PhantomData,
    }
  }

  /// Gets a chunk reference for reading.
  #[inline]
  pub fn get(&self, pos: ChunkPos) -> Option<&Chunk> {
    self.chunks.get(&pos).map(|ptr| unsafe { &**ptr })
  }

  /// Gets a mutable chunk reference for writing.
  #[inline]
  pub fn get_mut(&self, pos: ChunkPos) -> Option<&mut Chunk> {
    self.chunks.get(&pos).map(|ptr| unsafe { &mut **ptr })
  }
}

/// Executes a blit operation across tiles in parallel using 2x2 checkerboard
/// scheduling.
pub fn parallel_blit<F>(
  chunks: &ChunkAccess<'_>,
  rect: WorldRect,
  f: F,
  dirty_chunks: &Mutex<HashSet<ChunkPos>>,
  dirty_tiles: Option<&Mutex<HashSet<TilePos>>>,
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
      process_tile(
        chunks,
        tile,
        &rect,
        &f,
        w_recip,
        h_recip,
        dirty_chunks,
        dirty_tiles,
      );
    });
    // Implicit barrier between phases due to sequential for loop
  }
}

/// Executes a simulation step across tiles in parallel using 2x2 checkerboard
/// scheduling.
///
/// For each pixel in each tile, calls `f(pos, chunks)` which returns:
/// - `Some(target)` to swap pos with target
/// - `None` to leave pixel unchanged
pub fn parallel_simulate<F>(
  chunks: &ChunkAccess<'_>,
  tiles_by_phase: [Vec<TilePos>; 4],
  f: F,
  dirty_chunks: &Mutex<HashSet<ChunkPos>>,
  debug_gizmos: DebugGizmos<'_>,
) where
  F: Fn(WorldPos, &ChunkAccess<'_>) -> Option<WorldPos> + Sync,
{
  for phase_tiles in tiles_by_phase {
    phase_tiles.par_iter().for_each(|&tile| {
      simulate_tile(chunks, tile, &f, dirty_chunks, debug_gizmos);
    });
    // Implicit barrier between phases due to sequential for loop
  }
}

/// Process a single tile for simulation, iterating bottom-to-top.
///
/// Only processes pixels within the tile's dirty rect bounds.
/// Resets the dirty rect before processing, then expands it as pixels change.
fn simulate_tile<F>(
  chunks: &ChunkAccess<'_>,
  tile: TilePos,
  f: &F,
  dirty_chunks: &Mutex<HashSet<ChunkPos>>,
  debug_gizmos: DebugGizmos<'_>,
) where
  F: Fn(WorldPos, &ChunkAccess<'_>) -> Option<WorldPos> + Sync,
{
  let tile_size = TILE_SIZE as i64;
  let base_x = tile.0 * tile_size;
  let base_y = tile.1 * tile_size;

  // Get chunk and tile-local coordinates for this tile
  let (chunk_pos, local_pos) = WorldPos(base_x, base_y).to_chunk_and_local();
  let tx = (local_pos.0 as u32) / TILE_SIZE;
  let ty = (local_pos.1 as u32) / TILE_SIZE;

  // Read and reset dirty rect
  let bounds = if let Some(chunk) = chunks.get_mut(chunk_pos) {
    let rect = chunk.tile_dirty_rect_mut(tx, ty);
    let b = rect.bounds();
    rect.reset();
    b
  } else {
    None
  };

  // Skip if no dirty pixels
  let Some((min_x, min_y, max_x, max_y)) = bounds else {
    return;
  };

  // Emit debug gizmo for this dirty rect
  debug_shim::emit_dirty_rect(debug_gizmos, tile, (min_x, min_y, max_x, max_y));

  // Track which chunks we've dirtied in this tile
  let mut local_dirty: HashSet<ChunkPos> = HashSet::new();
  // Track pixels to mark dirty for next pass
  let mut dirty_pixels: Vec<(ChunkPos, LocalPos)> = Vec::new();

  // Process pixels bottom-to-top so falling sand settles correctly
  // Only iterate within dirty bounds
  for local_y in (min_y as i64)..=(max_y as i64) {
    // Alternate left-to-right and right-to-left per row for more natural flow
    let go_left = (tile.0 + local_y) % 2 == 0;

    let x_range: Box<dyn Iterator<Item = i64>> = if go_left {
      Box::new((min_x as i64..=max_x as i64).rev())
    } else {
      Box::new(min_x as i64..=max_x as i64)
    };

    for local_x in x_range {
      let pos = WorldPos(base_x + local_x, base_y + local_y);

      if let Some(target) = f(pos, chunks)
        && let Some(dirty) = swap_pixels(chunks, pos, target)
      {
        local_dirty.extend(dirty);

        // Collect pixel positions for dirty rect expansion
        let (chunk_a, local_a) = pos.to_chunk_and_local();
        let (chunk_b, local_b) = target.to_chunk_and_local();
        dirty_pixels.push((chunk_a, local_a));
        dirty_pixels.push((chunk_b, local_b));

        // Wake up neighbors of the vacated position so they can fall
        // into the now-empty space
        for neighbor in [
          WorldPos(pos.0, pos.1 + 1),     // above
          WorldPos(pos.0 - 1, pos.1 + 1), // above-left
          WorldPos(pos.0 + 1, pos.1 + 1), // above-right
          WorldPos(pos.0 - 1, pos.1),     // left
          WorldPos(pos.0 + 1, pos.1),     // right
        ] {
          let (n_chunk, n_local) = neighbor.to_chunk_and_local();
          dirty_pixels.push((n_chunk, n_local));
        }
      }
    }
  }

  // Mark swapped pixels dirty for next pass
  for (pixel_chunk_pos, local) in dirty_pixels {
    if let Some(chunk) = chunks.get_mut(pixel_chunk_pos) {
      chunk.mark_pixel_dirty(local.0 as u32, local.1 as u32);
    }
  }

  // Merge local dirty set into global
  if !local_dirty.is_empty()
    && let Ok(mut global_dirty) = dirty_chunks.lock()
  {
    global_dirty.extend(local_dirty);
  }
}

/// Swaps two pixels at the given world positions.
///
/// Returns the chunk positions that were modified, or None if swap failed.
fn swap_pixels(chunks: &ChunkAccess<'_>, a: WorldPos, b: WorldPos) -> Option<[ChunkPos; 2]> {
  let (chunk_a, local_a) = a.to_chunk_and_local();
  let (chunk_b, local_b) = b.to_chunk_and_local();

  let la = (local_a.0 as u32, local_a.1 as u32);
  let lb = (local_b.0 as u32, local_b.1 as u32);

  if chunk_a == chunk_b {
    // Same chunk - direct access
    let chunk = chunks.get_mut(chunk_a)?;
    let pixel_a = chunk.pixels[la];
    let pixel_b = chunk.pixels[lb];
    chunk.pixels[la] = pixel_b;
    chunk.pixels[lb] = pixel_a;
    Some([chunk_a, chunk_a])
  } else {
    // Different chunks - get both
    // SAFETY: Checkerboard scheduling guarantees no overlapping access
    let chunk_ptr_a = chunks.chunks.get(&chunk_a)?;
    let chunk_ptr_b = chunks.chunks.get(&chunk_b)?;

    let chunk_ref_a = unsafe { &mut **chunk_ptr_a };
    let chunk_ref_b = unsafe { &mut **chunk_ptr_b };

    let pixel_a = chunk_ref_a.pixels[la];
    let pixel_b = chunk_ref_b.pixels[lb];
    chunk_ref_a.pixels[la] = pixel_b;
    chunk_ref_b.pixels[lb] = pixel_a;

    Some([chunk_a, chunk_b])
  }
}

/// Process a single tile, writing pixels that pass the filter.
fn process_tile<F>(
  chunks: &ChunkAccess<'_>,
  tile: TilePos,
  rect: &WorldRect,
  f: &F,
  w_recip: f32,
  h_recip: f32,
  dirty_chunks: &Mutex<HashSet<ChunkPos>>,
  dirty_tiles: Option<&Mutex<HashSet<TilePos>>>,
) where
  F: Fn(WorldFragment) -> Option<Pixel> + Sync,
{
  let tile_size = TILE_SIZE as i64;
  let tile_x_start = tile.0 * tile_size;
  let tile_y_start = tile.1 * tile_size;

  // Track which chunks we've dirtied in this tile
  let mut local_dirty: HashSet<ChunkPos> = HashSet::new();
  // Track pixels to mark dirty for simulation
  let mut dirty_pixels: Vec<(ChunkPos, LocalPos)> = Vec::new();

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
        if let Some(chunk) = chunks.get_mut(chunk_pos) {
          chunk.pixels[(local_pos.0 as u32, local_pos.1 as u32)] = pixel;
          local_dirty.insert(chunk_pos);
          dirty_pixels.push((chunk_pos, local_pos));
        }
      }
    }
  }

  // Mark painted pixels dirty for simulation
  for (pixel_chunk_pos, local) in dirty_pixels {
    if let Some(chunk) = chunks.get_mut(pixel_chunk_pos) {
      chunk.mark_pixel_dirty(local.0 as u32, local.1 as u32);
    }
  }

  // Merge local dirty set into global
  if !local_dirty.is_empty() {
    if let Ok(mut global_dirty) = dirty_chunks.lock() {
      global_dirty.extend(local_dirty);
    }

    // Track this tile as dirty
    if let Some(tiles_mutex) = dirty_tiles {
      if let Ok(mut tiles) = tiles_mutex.lock() {
        tiles.insert(tile);
      }
    }
  }
}
