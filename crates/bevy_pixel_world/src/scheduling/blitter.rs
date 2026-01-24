//! Parallel tile blitter with 2x2 checkerboard scheduling.
//!
//! This module solves the cross-chunk boundary problem for parallel pixel operations.
//! When processing falling sand physics or painting operations, adjacent pixels may
//! swap across chunk boundaries. Naive parallelization would cause data races.
//!
//! # Checkerboard Scheduling
//!
//! Tiles are grouped into four phases (A, B, C, D) based on their position modulo 2:
//!
//! ```text
//! ┌───┬───┬───┬───┐
//! │ A │ B │ A │ B │
//! ├───┼───┼───┼───┤
//! │ C │ D │ C │ D │
//! ├───┼───┼───┼───┤
//! │ A │ B │ A │ B │
//! └───┴───┴───┴───┘
//! ```
//!
//! Tiles in the same phase are never adjacent (horizontally, vertically, or diagonally),
//! guaranteeing that parallel threads cannot access overlapping pixel regions. Each phase
//! is processed sequentially, with tiles within that phase processed in parallel.
//!
//! # Data Hierarchy
//!
//! - [`Canvas`] - Unified view over multiple chunks for cross-boundary access
//! - `HashMap<ChunkPos, &mut Chunk>` - The underlying chunk storage
//! - `Chunk::pixels: Surface<Pixel>` - Per-chunk pixel data
//!
//! The Canvas provides safe mutable access to multiple chunks by leveraging the
//! checkerboard invariant: since tiles in the same phase never overlap, raw pointer
//! access is sound.
//!
//! # Key Functions
//!
//! - [`parallel_blit`] - Paint operations with custom pixel shaders
//! - [`parallel_simulate`] - Cellular automata physics simulation
//!
//! See `docs/architecture/scheduling.md` for detailed design rationale.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use rayon::prelude::*;

use crate::coords::{ChunkPos, LocalPos, Phase, TilePos, WorldFragment, WorldPos, WorldRect, TILE_SIZE};
use crate::debug_shim::{self, DebugGizmos};
use crate::pixel::Pixel;
use crate::primitives::Chunk;
use crate::simulation::hash::hash21uu64;

/// Unified drawing surface spanning multiple chunks.
///
/// Provides a single coordinate space for pixel operations across chunk
/// boundaries, used by both painting (blit) and simulation.
///
/// # Safety
/// This type provides interior mutability without runtime checks.
/// It is only safe to use with the 2x2 checkerboard scheduling, which
/// guarantees tiles in the same phase never access overlapping pixels.
pub struct Canvas<'a> {
  chunks: HashMap<ChunkPos, *mut Chunk>,
  _marker: std::marker::PhantomData<&'a mut Chunk>,
}

// SAFETY: The 2x2 checkerboard scheduling guarantees that tiles processed
// in parallel never access overlapping pixel regions.
unsafe impl Send for Canvas<'_> {}
unsafe impl Sync for Canvas<'_> {}

impl<'a> Canvas<'a> {
  /// Creates a canvas from mutable chunk references.
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

  /// Gets mutable references to two different chunks.
  ///
  /// Returns None if either chunk is not found. Panics if `a == b`
  /// (use `get_mut` for same-chunk access).
  ///
  /// # Safety
  /// This is safe because the positions are guaranteed to be different,
  /// so the raw pointers point to distinct memory locations.
  #[inline]
  pub fn get_two_mut(&self, a: ChunkPos, b: ChunkPos) -> Option<(&mut Chunk, &mut Chunk)> {
    debug_assert_ne!(a, b, "get_two_mut requires different chunk positions");
    let ptr_a = self.chunks.get(&a)?;
    let ptr_b = self.chunks.get(&b)?;
    // SAFETY: a != b guarantees these are distinct memory locations.
    // Checkerboard scheduling guarantees no overlapping pixel access.
    Some(unsafe { (&mut **ptr_a, &mut **ptr_b) })
  }
}

/// Executes a blit operation across tiles in parallel using 2x2 checkerboard
/// scheduling.
pub fn parallel_blit<F>(
  chunks: &Canvas<'_>,
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
    phases[phase.index()].push(tile);
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
///
/// The `jitter` parameter offsets the tile grid each tick so that tile
/// boundaries appear at different world positions, preventing artifacts
/// from accumulating at fixed seams.
pub fn parallel_simulate<F>(
  chunks: &Canvas<'_>,
  tiles_by_phase: [Vec<TilePos>; 4],
  f: F,
  dirty_chunks: &Mutex<HashSet<ChunkPos>>,
  debug_gizmos: DebugGizmos<'_>,
  tick: u64,
  jitter: (i64, i64),
) where
  F: Fn(WorldPos, &Canvas<'_>) -> Option<WorldPos> + Sync,
{
  #[cfg(feature = "tracy")]
  let _span = tracing::info_span!("parallel_simulate").entered();

  for (_phase_idx, phase_tiles) in tiles_by_phase.iter().enumerate() {
    #[cfg(feature = "tracy")]
    let _phase_span = tracing::info_span!("phase", phase = _phase_idx).entered();

    phase_tiles.par_iter().for_each(|&tile| {
      simulate_tile(chunks, tile, &f, dirty_chunks, debug_gizmos, tick, jitter);
    });
    // Implicit barrier between phases due to sequential for loop
  }
}

/// Process a single tile for simulation, iterating bottom-to-top.
///
/// Only processes pixels within the tile's dirty rect bounds.
/// Resets the dirty rect before processing, then expands it as pixels change.
///
/// With jitter, the tile grid is offset so tile boundaries appear at different
/// world positions each tick. The jittered tile overlaps 1-4 original tiles,
/// so we union their dirty bounds for iteration but still mark dirty using
/// original (non-jittered) tile coordinates.
fn simulate_tile<F>(
  chunks: &Canvas<'_>,
  tile: TilePos,
  f: &F,
  dirty_chunks: &Mutex<HashSet<ChunkPos>>,
  debug_gizmos: DebugGizmos<'_>,
  tick: u64,
  jitter: (i64, i64),
) where
  F: Fn(WorldPos, &Canvas<'_>) -> Option<WorldPos> + Sync,
{
  let tile_size = TILE_SIZE as i64;
  let (jitter_x, jitter_y) = jitter;

  // Jittered base position - where this tile's pixels actually start
  let base_x = tile.x * tile_size + jitter_x;
  let base_y = tile.y * tile_size + jitter_y;

  // Tick the "owned" original tile (same index as jittered tile)
  // This maintains the dirty rect state machine correctly
  let orig_base_x = tile.x * tile_size;
  let orig_base_y = tile.y * tile_size;
  let (chunk_pos, local_pos) = WorldPos::new(orig_base_x, orig_base_y).to_chunk_and_local();
  let tx = (local_pos.x as u32) / TILE_SIZE;
  let ty = (local_pos.y as u32) / TILE_SIZE;

  if let Some(chunk) = chunks.get_mut(chunk_pos) {
    chunk.tile_dirty_rect_mut(tx, ty).tick();
  }

  // Union dirty bounds from all overlapping original tiles
  let bounds = union_dirty_bounds(chunks, tile, jitter);

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
    // Alternate direction per row using hash for temporal variation
    let world_y = base_y + local_y;
    let go_left = hash21uu64(tick, world_y as u64) & 1 == 0;

    let x_range: Box<dyn Iterator<Item = i64>> = if go_left {
      Box::new((min_x as i64..=max_x as i64).rev())
    } else {
      Box::new(min_x as i64..=max_x as i64)
    };

    for local_x in x_range {
      let pos = WorldPos::new(base_x + local_x, base_y + local_y);

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
          WorldPos::new(pos.x, pos.y + 1),     // above
          WorldPos::new(pos.x - 1, pos.y + 1), // above-left
          WorldPos::new(pos.x + 1, pos.y + 1), // above-right
          WorldPos::new(pos.x - 1, pos.y),     // left
          WorldPos::new(pos.x + 1, pos.y),     // right
        ] {
          let (n_chunk, n_local) = neighbor.to_chunk_and_local();
          dirty_pixels.push((n_chunk, n_local));
        }
      }
    }
  }

  // Mark swapped pixels dirty for next pass
  // Uses original (non-jittered) tile coordinates via chunk.mark_pixel_dirty
  for (pixel_chunk_pos, local) in dirty_pixels {
    if let Some(chunk) = chunks.get_mut(pixel_chunk_pos) {
      chunk.mark_pixel_dirty(local.x as u32, local.y as u32);
    }
  }

  // Merge local dirty set into global
  if !local_dirty.is_empty()
    && let Ok(mut global_dirty) = dirty_chunks.lock()
  {
    global_dirty.extend(local_dirty);
  }
}

/// Compute the union of dirty bounds from all original tiles that overlap
/// a jittered tile.
///
/// A jittered tile at (tx, ty) with jitter (jx, jy) overlaps:
/// - Original tile (tx, ty) - always
/// - Original tile (tx+1, ty) - if jx > 0
/// - Original tile (tx, ty+1) - if jy > 0
/// - Original tile (tx+1, ty+1) - if jx > 0 and jy > 0
fn union_dirty_bounds(
  chunks: &Canvas<'_>,
  tile: TilePos,
  jitter: (i64, i64),
) -> Option<(u8, u8, u8, u8)> {
  let tile_size = TILE_SIZE as i64;
  let (jitter_x, jitter_y) = jitter;

  // Jittered tile base position
  let jittered_base_x = tile.x * tile_size + jitter_x;
  let jittered_base_y = tile.y * tile_size + jitter_y;

  let mut union_bounds: Option<(u8, u8, u8, u8)> = None;

  // Check up to 4 overlapping original tiles
  for dy in 0i64..=1 {
    if dy == 1 && jitter_y == 0 {
      continue;
    }
    for dx in 0i64..=1 {
      if dx == 1 && jitter_x == 0 {
        continue;
      }

      let orig_tile = TilePos::new(tile.x + dx, tile.y + dy);
      let orig_base_x = orig_tile.x * tile_size;
      let orig_base_y = orig_tile.y * tile_size;

      // Get chunk and tile-local coordinates for this original tile
      let (chunk_pos, local_pos) = WorldPos::new(orig_base_x, orig_base_y).to_chunk_and_local();
      let tx = (local_pos.x as u32) / TILE_SIZE;
      let ty = (local_pos.y as u32) / TILE_SIZE;

      let Some(chunk) = chunks.get(chunk_pos) else {
        continue;
      };

      let Some((min_x, min_y, max_x, max_y)) = chunk.tile_dirty_rect(tx, ty).bounds() else {
        continue;
      };

      // Convert original tile dirty bounds to world coords
      let world_min_x = orig_base_x + min_x as i64;
      let world_min_y = orig_base_y + min_y as i64;
      let world_max_x = orig_base_x + max_x as i64;
      let world_max_y = orig_base_y + max_y as i64;

      // Convert to jittered tile local coords and clamp to [0, 31]
      let local_min_x = (world_min_x - jittered_base_x).clamp(0, 31) as u8;
      let local_min_y = (world_min_y - jittered_base_y).clamp(0, 31) as u8;
      let local_max_x = (world_max_x - jittered_base_x).clamp(0, 31) as u8;
      let local_max_y = (world_max_y - jittered_base_y).clamp(0, 31) as u8;

      // Skip if this results in empty rect (can happen when dirty region
      // doesn't actually overlap jittered tile)
      if local_min_x > local_max_x || local_min_y > local_max_y {
        continue;
      }

      // Union with accumulated bounds
      union_bounds = Some(match union_bounds {
        None => (local_min_x, local_min_y, local_max_x, local_max_y),
        Some((u_min_x, u_min_y, u_max_x, u_max_y)) => (
          u_min_x.min(local_min_x),
          u_min_y.min(local_min_y),
          u_max_x.max(local_max_x),
          u_max_y.max(local_max_y),
        ),
      });
    }
  }

  union_bounds
}

/// Marks a pixel position as collision dirty if the material changed.
///
/// We mark dirty if either pixel is non-void, since collision depends on
/// material state which we don't have access to here. The collision system
/// will determine actual collision status using the material registry.
///
/// Also marks adjacent tiles dirty when the pixel is at a tile boundary,
/// since collision meshes sample a 1px border from neighbors.
#[inline]
fn mark_collision_dirty_if_changed(chunk: &mut Chunk, local_x: u32, local_y: u32, old: &Pixel, new: &Pixel) {
  // If either old or new is non-void, mark dirty (material may have collision)
  // This is conservative but correct - the collision system will filter further
  if !old.is_void() || !new.is_void() {
    let tx = local_x / TILE_SIZE;
    let ty = local_y / TILE_SIZE;
    chunk.mark_tile_collision_dirty(tx, ty);

    // Also mark adjacent tiles if pixel is at tile boundary
    // (collision meshes include 1px border from neighbors)
    let px = local_x % TILE_SIZE;
    let py = local_y % TILE_SIZE;
    let max_local = TILE_SIZE - 1;
    let tiles_per_chunk = crate::coords::TILES_PER_CHUNK;

    // Left boundary: also mark tile to the left
    if px == 0 && tx > 0 {
      chunk.mark_tile_collision_dirty(tx - 1, ty);
    }

    // Right boundary: also mark tile to the right
    if px == max_local && tx + 1 < tiles_per_chunk {
      chunk.mark_tile_collision_dirty(tx + 1, ty);
    }

    // Bottom boundary: also mark tile below
    if py == 0 && ty > 0 {
      chunk.mark_tile_collision_dirty(tx, ty - 1);
    }

    // Top boundary: also mark tile above
    if py == max_local && ty + 1 < tiles_per_chunk {
      chunk.mark_tile_collision_dirty(tx, ty + 1);
    }

    // Corner cases: mark diagonal tiles if at corner
    if px == 0 && py == 0 && tx > 0 && ty > 0 {
      chunk.mark_tile_collision_dirty(tx - 1, ty - 1);
    }
    if px == max_local && py == 0 && tx + 1 < tiles_per_chunk && ty > 0 {
      chunk.mark_tile_collision_dirty(tx + 1, ty - 1);
    }
    if px == 0 && py == max_local && tx > 0 && ty + 1 < tiles_per_chunk {
      chunk.mark_tile_collision_dirty(tx - 1, ty + 1);
    }
    if px == max_local && py == max_local && tx + 1 < tiles_per_chunk && ty + 1 < tiles_per_chunk {
      chunk.mark_tile_collision_dirty(tx + 1, ty + 1);
    }
  }
}

/// Swaps two pixels at the given world positions.
///
/// Returns the chunk positions that were modified, or None if swap failed.
fn swap_pixels(chunks: &Canvas<'_>, a: WorldPos, b: WorldPos) -> Option<[ChunkPos; 2]> {
  let (chunk_a, local_a) = a.to_chunk_and_local();
  let (chunk_b, local_b) = b.to_chunk_and_local();

  let la = (local_a.x as u32, local_a.y as u32);
  let lb = (local_b.x as u32, local_b.y as u32);

  if chunk_a == chunk_b {
    // Same chunk - direct access
    let chunk = chunks.get_mut(chunk_a)?;
    let pixel_a = chunk.pixels[la];
    let pixel_b = chunk.pixels[lb];

    // Mark collision dirty if collision state changes
    mark_collision_dirty_if_changed(chunk, la.0, la.1, &pixel_a, &pixel_b);
    mark_collision_dirty_if_changed(chunk, lb.0, lb.1, &pixel_b, &pixel_a);

    chunk.pixels[la] = pixel_b;
    chunk.pixels[lb] = pixel_a;
    Some([chunk_a, chunk_a])
  } else {
    // Different chunks - get both via encapsulated method
    let (chunk_ref_a, chunk_ref_b) = chunks.get_two_mut(chunk_a, chunk_b)?;

    let pixel_a = chunk_ref_a.pixels[la];
    let pixel_b = chunk_ref_b.pixels[lb];

    // Mark collision dirty if collision state changes
    mark_collision_dirty_if_changed(chunk_ref_a, la.0, la.1, &pixel_a, &pixel_b);
    mark_collision_dirty_if_changed(chunk_ref_b, lb.0, lb.1, &pixel_b, &pixel_a);

    chunk_ref_a.pixels[la] = pixel_b;
    chunk_ref_b.pixels[lb] = pixel_a;

    Some([chunk_a, chunk_b])
  }
}

/// Process a single tile, writing pixels that pass the filter.
fn process_tile<F>(
  chunks: &Canvas<'_>,
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
  let tile_x_start = tile.x * tile_size;
  let tile_y_start = tile.y * tile_size;

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
      if let Some(new_pixel) = f(frag) {
        // Convert world pos to chunk + local
        let (chunk_pos, local_pos) = WorldPos::new(world_x, world_y).to_chunk_and_local();

        // Try to write the pixel
        if let Some(chunk) = chunks.get_mut(chunk_pos) {
          let lx = local_pos.x as u32;
          let ly = local_pos.y as u32;
          let old_pixel = chunk.pixels[(lx, ly)];

          // Mark collision dirty if collision state changes
          mark_collision_dirty_if_changed(chunk, lx, ly, &old_pixel, &new_pixel);

          chunk.pixels[(lx, ly)] = new_pixel;
          local_dirty.insert(chunk_pos);
          dirty_pixels.push((chunk_pos, local_pos));
        }
      }
    }
  }

  // Mark painted pixels dirty for simulation
  for (pixel_chunk_pos, local) in dirty_pixels {
    if let Some(chunk) = chunks.get_mut(pixel_chunk_pos) {
      chunk.mark_pixel_dirty(local.x as u32, local.y as u32);
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
