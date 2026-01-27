//! Parallel blit orchestration for `PixelWorld`.
//!
//! The `blit` method applies a shader-style callback across a world-space
//! rectangle, using 2x2 checkerboard scheduling for thread-safe writes.

use std::collections::HashMap;

use super::PixelWorld;
use crate::coords::{ChunkPos, TilePos, WorldFragment, WorldRect};
use crate::debug_shim::{self, DebugGizmos};
use crate::pixel::Pixel;
use crate::primitives::Chunk;
use crate::scheduling::blitter::{Canvas, parallel_blit};

impl PixelWorld {
  /// Collects mutable references to all seeded chunks for parallel access.
  ///
  /// Delegates to ChunkPool which encapsulates the unsafe pointer handling.
  pub(crate) fn collect_seeded_chunks(&mut self) -> HashMap<ChunkPos, &mut Chunk> {
    self.pool.collect_seeded_mut()
  }

  /// Blits pixels using a shader-style callback.
  ///
  /// For each pixel in `rect`, calls `f(fragment)` where fragment contains
  /// world coordinates and normalized UV. If `f` returns Some(pixel), that
  /// pixel is written; if None, the pixel is unchanged.
  ///
  /// Uses parallel 2x2 checkerboard scheduling for thread-safe concurrent
  /// writes. Returns the list of chunk positions that were modified.
  ///
  /// The `debug_gizmos` parameter emits visual debug overlays when the
  /// `visual-debug` feature is enabled. Pass `()` when disabled.
  pub fn blit<F>(&mut self, rect: WorldRect, f: F, debug_gizmos: DebugGizmos<'_>) -> Vec<ChunkPos>
  where
    F: Fn(WorldFragment) -> Option<Pixel> + Sync,
  {
    let chunks = self.collect_seeded_chunks();
    let chunk_access = Canvas::new(chunks);
    let dirty_chunks = std::sync::Mutex::new(std::collections::HashSet::new());
    let dirty_tiles = std::sync::Mutex::new(std::collections::HashSet::<TilePos>::new());

    parallel_blit(&chunk_access, rect, f, &dirty_chunks, Some(&dirty_tiles));

    let dirty: Vec<_> = dirty_chunks
      .into_inner()
      .unwrap_or_default()
      .into_iter()
      .collect();
    let dirty_tile_list: Vec<_> = dirty_tiles
      .into_inner()
      .unwrap_or_default()
      .into_iter()
      .collect();

    // Mark affected chunks as dirty and needing save
    for &pos in &dirty {
      if let Some(idx) = self.pool.index_for(pos) {
        let slot = self.pool.get_mut(idx);
        slot.dirty = true;
        slot.modified = true;
        slot.persisted = false;
      }
    }

    // Emit debug gizmos
    debug_shim::emit_blit_rect(debug_gizmos, rect);
    for &pos in &dirty {
      debug_shim::emit_chunk(debug_gizmos, pos);
    }
    for &tile in &dirty_tile_list {
      debug_shim::emit_tile(debug_gizmos, tile);
    }

    dirty
  }
}
