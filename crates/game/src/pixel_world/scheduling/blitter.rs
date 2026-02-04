//! Parallel tile blitter with 2x2 checkerboard scheduling.
//!
//! This module solves the cross-chunk boundary problem for parallel pixel
//! operations. When processing falling sand physics or painting operations,
//! adjacent pixels may swap across chunk boundaries. Naive parallelization
//! would cause data races.
//!
//! # Checkerboard Scheduling
//!
//! Tiles are grouped into four phases (A, B, C, D) based on their position
//! modulo 2:
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
//! Tiles in the same phase are never adjacent (horizontally, vertically, or
//! diagonally), guaranteeing that parallel threads cannot access overlapping
//! pixel regions. Each phase is processed sequentially, with tiles within that
//! phase processed in parallel.
//!
//! # Data Hierarchy
//!
//! - [`Canvas`] - Unified view over multiple chunks for cross-boundary access
//! - `HashMap<ChunkPos, &mut Chunk>` - The underlying chunk storage
//! - `Chunk::pixels: Surface<Pixel>` - Per-chunk pixel data
//!
//! The Canvas provides safe mutable access to multiple chunks by leveraging the
//! checkerboard invariant: since tiles in the same phase never overlap, raw
//! pointer access is sound.
//!
//! # Key Functions
//!
//! - [`parallel_blit`] - Paint operations with custom pixel shaders
//! - [`parallel_simulate`] - Cellular automata physics simulation
//!
//! See `docs/architecture/scheduling.md` for detailed design rationale.

use std::collections::HashSet;
use std::sync::Mutex;

use rayon::prelude::*;

pub use super::canvas::Canvas;
use super::checkerboard::{
  WAKE_NEIGHBORS, adjacent_tiles_at_boundary, mark_pixels_dirty, tick_owned_tile,
  union_dirty_bounds,
};
use crate::pixel_world::coords::{
  ChunkPos, LocalPos, Phase, TILE_SIZE, TilePos, WorldFragment, WorldPos, WorldRect,
};
use crate::pixel_world::debug_shim::{self, DebugGizmos};
use crate::pixel_world::pixel::Pixel;
use crate::pixel_world::primitives::Chunk;
use crate::pixel_world::simulation::burning::{self, BurningContext};
use crate::pixel_world::simulation::hash::hash21uu64;

/// Context for tile-based blit operations.
///
/// Bundles parameters needed by process_tile to reduce function signature
/// complexity.
struct TileContext<'a> {
  rect: &'a WorldRect,
  w_recip: f32,
  h_recip: f32,
  dirty_chunks: &'a Mutex<HashSet<ChunkPos>>,
  dirty_tiles: Option<&'a Mutex<HashSet<TilePos>>>,
}

/// Context for tile-based simulation operations.
///
/// Bundles parameters needed by simulate_tile to reduce function signature
/// complexity.
struct SimulationContext<'a> {
  dirty_chunks: &'a Mutex<HashSet<ChunkPos>>,
  debug_gizmos: DebugGizmos<'a>,
  tick: u64,
  jitter: (i64, i64),
}

/// Collects dirty state during tile processing.
///
/// Groups the three dirty tracking mechanisms:
/// - Global chunk set (mutex-protected, for GPU upload)
/// - Local chunk set (per-tile accumulator)
/// - Pixel list (for dirty rect expansion)
struct DirtyCollector<'a> {
  global_chunks: &'a Mutex<HashSet<ChunkPos>>,
  local_chunks: HashSet<ChunkPos>,
  pixels: Vec<(ChunkPos, LocalPos)>,
}

impl<'a> DirtyCollector<'a> {
  fn new(global_chunks: &'a Mutex<HashSet<ChunkPos>>) -> Self {
    Self {
      global_chunks,
      local_chunks: HashSet::new(),
      pixels: Vec::new(),
    }
  }

  /// Flushes local dirty state to global and marks pixels for next pass.
  fn flush(self, chunks: &Canvas<'_>) {
    mark_pixels_dirty(chunks, &self.pixels);

    if !self.local_chunks.is_empty()
      && let Ok(mut global) = self.global_chunks.lock()
    {
      global.extend(self.local_chunks);
    }
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

  let ctx = TileContext {
    rect: &rect,
    w_recip,
    h_recip,
    dirty_chunks,
    dirty_tiles,
  };

  // Execute each phase sequentially, tiles within phase in parallel
  for phase_tiles in phases {
    phase_tiles.par_iter().for_each(|&tile| {
      process_tile(chunks, tile, &ctx, &f);
    });
    // Implicit barrier between phases due to sequential for loop
  }
}

/// Executes a simulation step across tiles in parallel using 2x2 checkerboard
/// scheduling.
///
/// For each pixel in each tile, calls `compute_swap(pos, chunks)` which
/// returns:
/// - `Some(target)` to swap pos with target
/// - `None` to leave pixel unchanged
///
/// The `jitter` parameter offsets the tile grid each tick so that tile
/// boundaries appear at different world positions, preventing artifacts
/// from accumulating at fixed seams.
pub fn parallel_simulate<F>(
  chunks: &Canvas<'_>,
  tiles_by_phase: [Vec<TilePos>; 4],
  compute_swap: F,
  dirty_chunks: &Mutex<HashSet<ChunkPos>>,
  debug_gizmos: DebugGizmos<'_>,
  tick: u64,
  jitter: (i64, i64),
) where
  F: Fn(WorldPos, &Canvas<'_>) -> Option<WorldPos> + Sync,
{
  #[cfg(feature = "tracy")]
  let _span = tracing::info_span!("parallel_simulate").entered();

  let ctx = SimulationContext {
    dirty_chunks,
    debug_gizmos,
    tick,
    jitter,
  };

  #[cfg(feature = "tracy")]
  let mut _phase_idx = 0usize;

  for phase_tiles in &tiles_by_phase {
    #[cfg(feature = "tracy")]
    let _phase_span = tracing::info_span!("phase", phase = _phase_idx).entered();

    phase_tiles.par_iter().for_each(|&tile| {
      simulate_tile(chunks, tile, &compute_swap, &ctx);
    });
    // Implicit barrier between phases due to sequential for loop

    #[cfg(feature = "tracy")]
    {
      _phase_idx += 1;
    }
  }
}

/// Executes burning propagation across tiles in parallel using 2x2 checkerboard
/// scheduling.
///
/// For each pixel in dirty bounds, checks if it's burning and processes
/// fire spread and burn effects. Uses the same tile/phase infrastructure
/// as physics simulation for thread safety.
pub fn parallel_burning(
  chunks: &Canvas<'_>,
  tiles_by_phase: [Vec<TilePos>; 4],
  burning_ctx: &BurningContext<'_>,
  dirty_chunks: &Mutex<HashSet<ChunkPos>>,
  jitter: (i64, i64),
) {
  #[cfg(feature = "tracy")]
  let _span = tracing::info_span!("parallel_burning").entered();

  for phase_tiles in &tiles_by_phase {
    phase_tiles.par_iter().for_each(|&tile| {
      burn_tile(chunks, tile, burning_ctx, dirty_chunks, jitter);
    });
  }
}

/// Process a single tile for burning propagation.
///
/// Only processes pixels within the tile's dirty rect bounds.
fn burn_tile(
  chunks: &Canvas<'_>,
  tile: TilePos,
  burning_ctx: &BurningContext<'_>,
  dirty_chunks: &Mutex<HashSet<ChunkPos>>,
  jitter: (i64, i64),
) {
  let Some(bounds) = union_dirty_bounds(chunks, tile, jitter) else {
    return;
  };

  let mut local_dirty_chunks = HashSet::new();
  let mut dirty_pixels = Vec::new();

  burning::process_tile_burning(
    chunks,
    tile,
    bounds,
    jitter,
    burning_ctx,
    &mut local_dirty_chunks,
    &mut dirty_pixels,
  );

  // Mark affected pixels dirty for next physics pass
  mark_pixels_dirty(chunks, &dirty_pixels);

  // Flush to global dirty set
  if !local_dirty_chunks.is_empty() {
    if let Ok(mut global) = dirty_chunks.lock() {
      global.extend(local_dirty_chunks);
    }
  }
}

/// Iterates over pixel positions within dirty bounds with row-alternating
/// direction.
///
/// The direction alternates per row based on a hash of tick and world_y,
/// preventing visual artifacts from accumulating at fixed seams.
fn for_each_pixel_in_bounds<F>(bounds: (u8, u8, u8, u8), base: (i64, i64), tick: u64, mut f: F)
where
  F: FnMut(WorldPos),
{
  let (min_x, min_y, max_x, max_y) = bounds;
  let (base_x, base_y) = base;

  for local_y in (min_y as i64)..=(max_y as i64) {
    let world_y = base_y + local_y;
    let go_left = hash21uu64(tick, world_y as u64) & 1 == 0;

    if go_left {
      for local_x in (min_x as i64..=max_x as i64).rev() {
        f(WorldPos::new(base_x + local_x, world_y));
      }
    } else {
      for local_x in min_x as i64..=max_x as i64 {
        f(WorldPos::new(base_x + local_x, world_y));
      }
    }
  }
}

/// Records the effects of a successful pixel swap.
///
/// This includes:
/// - Extending the local dirty chunk set with affected chunks
/// - Recording both swapped positions for dirty rect expansion
/// - Waking neighbor pixels above and to the sides of the vacated position
fn record_swap_effects(
  pos: WorldPos,
  target: WorldPos,
  dirty_chunks: [ChunkPos; 2],
  local_dirty: &mut HashSet<ChunkPos>,
  dirty_pixels: &mut Vec<(ChunkPos, LocalPos)>,
) {
  local_dirty.extend(dirty_chunks);

  let (chunk_a, local_a) = pos.to_chunk_and_local();
  let (chunk_b, local_b) = target.to_chunk_and_local();
  dirty_pixels.push((chunk_a, local_a));
  dirty_pixels.push((chunk_b, local_b));

  for (dx, dy) in WAKE_NEIGHBORS {
    let neighbor = WorldPos::new(pos.x + dx, pos.y + dy);
    let (n_chunk, n_local) = neighbor.to_chunk_and_local();
    dirty_pixels.push((n_chunk, n_local));
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
  compute_swap: &F,
  ctx: &SimulationContext<'_>,
) where
  F: Fn(WorldPos, &Canvas<'_>) -> Option<WorldPos> + Sync,
{
  let tile_size = TILE_SIZE as i64;
  let base = (
    tile.x * tile_size + ctx.jitter.0,
    tile.y * tile_size + ctx.jitter.1,
  );

  tick_owned_tile(chunks, tile);

  let Some(bounds) = union_dirty_bounds(chunks, tile, ctx.jitter) else {
    return;
  };

  debug_shim::emit_dirty_rect(ctx.debug_gizmos, tile, bounds);

  let mut collector = DirtyCollector::new(ctx.dirty_chunks);

  for_each_pixel_in_bounds(bounds, base, ctx.tick, |pos| {
    if let Some(target) = compute_swap(pos, chunks)
      && let Some(dirty) = swap_pixels(chunks, pos, target)
    {
      record_swap_effects(
        pos,
        target,
        dirty,
        &mut collector.local_chunks,
        &mut collector.pixels,
      );
    }
  });

  collector.flush(chunks);
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
fn mark_collision_dirty_if_changed(
  chunk: &mut Chunk,
  local_x: u32,
  local_y: u32,
  old: &Pixel,
  new: &Pixel,
) {
  // If either old or new is non-void, mark dirty (material may have collision)
  // This is conservative but correct - the collision system will filter further
  if !old.is_void() || !new.is_void() {
    let tx = local_x / TILE_SIZE;
    let ty = local_y / TILE_SIZE;
    chunk.mark_tile_collision_dirty(tx, ty);

    // Mark adjacent tiles if pixel is at tile boundary
    let px = local_x % TILE_SIZE;
    let py = local_y % TILE_SIZE;
    for (adj_tx, adj_ty) in adjacent_tiles_at_boundary(px, py, tx, ty) {
      chunk.mark_tile_collision_dirty(adj_tx, adj_ty);
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

/// Writes a pixel to the canvas, handling collision marking and dirty tracking.
///
/// Returns true if the pixel was written.
fn write_pixel(
  chunks: &Canvas<'_>,
  world_pos: WorldPos,
  new_pixel: Pixel,
  local_dirty: &mut HashSet<ChunkPos>,
  dirty_pixels: &mut Vec<(ChunkPos, LocalPos)>,
) -> bool {
  let (chunk_pos, local_pos) = world_pos.to_chunk_and_local();

  let Some(chunk) = chunks.get_mut(chunk_pos) else {
    return false;
  };

  let lx = local_pos.x as u32;
  let ly = local_pos.y as u32;
  let old_pixel = chunk.pixels[(lx, ly)];

  mark_collision_dirty_if_changed(chunk, lx, ly, &old_pixel, &new_pixel);
  chunk.pixels[(lx, ly)] = new_pixel;
  local_dirty.insert(chunk_pos);
  dirty_pixels.push((chunk_pos, local_pos));

  true
}

/// Process a single tile, writing pixels that pass the filter.
fn process_tile<F>(chunks: &Canvas<'_>, tile: TilePos, ctx: &TileContext<'_>, f: &F)
where
  F: Fn(WorldFragment) -> Option<Pixel> + Sync,
{
  let Some((min_dx, max_dx, min_dy, max_dy)) = ctx.rect.clip_tile(tile) else {
    return;
  };

  let tile_size = TILE_SIZE as i64;
  let tile_x_start = tile.x * tile_size;
  let tile_y_start = tile.y * tile_size;

  let mut collector = DirtyCollector::new(ctx.dirty_chunks);

  for dy in min_dy..=max_dy {
    let world_y = tile_y_start + dy as i64;

    for dx in min_dx..=max_dx {
      let world_x = tile_x_start + dx as i64;

      let u = (world_x - ctx.rect.x) as f32 * ctx.w_recip;
      let v = (world_y - ctx.rect.y) as f32 * ctx.h_recip;

      let frag = WorldFragment {
        x: world_x,
        y: world_y,
        u,
        v,
      };

      if let Some(new_pixel) = f(frag) {
        write_pixel(
          chunks,
          WorldPos::new(world_x, world_y),
          new_pixel,
          &mut collector.local_chunks,
          &mut collector.pixels,
        );
      }
    }
  }

  // Track dirty tiles separately (blit-specific, not in DirtyCollector)
  if !collector.local_chunks.is_empty()
    && let Some(tiles_mutex) = ctx.dirty_tiles
    && let Ok(mut tiles) = tiles_mutex.lock()
  {
    tiles.insert(tile);
  }

  collector.flush(chunks);
}
