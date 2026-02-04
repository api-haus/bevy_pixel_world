//! Pixel read/write API for `PixelWorld`.
//!
//! These methods provide world-coordinate pixel access, translating
//! `WorldPos` to chunk+local coordinates and resolving through the pool.

use super::PixelWorld;
use crate::pixel_world::coords::WorldPos;
use crate::pixel_world::debug_shim::{self, DebugGizmos};
use crate::pixel_world::pixel::Pixel;
use crate::pixel_world::primitives::HEAT_CELL_SIZE;

impl PixelWorld {
  /// Returns a reference to the pixel at the given world position.
  ///
  /// Returns None if the chunk is not loaded or not yet seeded.
  pub fn get_pixel(&self, pos: WorldPos) -> Option<&Pixel> {
    let (chunk_pos, local_pos) = pos.to_chunk_and_local();
    let idx = self.pool.index_for(chunk_pos)?;
    let slot = self.pool.get(idx);
    if !slot.is_seeded() {
      return None;
    }
    Some(&slot.chunk.pixels[(local_pos.x as u32, local_pos.y as u32)])
  }

  /// Returns a mutable reference to the pixel at the given world position.
  ///
  /// Returns None if the chunk is not loaded or not yet seeded.
  /// Does NOT mark the chunk as dirty - caller must do so.
  pub fn get_pixel_mut(&mut self, pos: WorldPos) -> Option<&mut Pixel> {
    let (chunk_pos, local_pos) = pos.to_chunk_and_local();
    let idx = self.pool.index_for(chunk_pos)?;
    let slot = self.pool.get_mut(idx);
    if !slot.is_seeded() {
      return None;
    }
    Some(&mut slot.chunk.pixels[(local_pos.x as u32, local_pos.y as u32)])
  }

  /// Swaps two pixels at the given world positions.
  ///
  /// Returns true if the swap was successful, false if either chunk
  /// is not loaded or not yet seeded.
  pub fn swap_pixels(&mut self, a: WorldPos, b: WorldPos) -> bool {
    let (chunk_a, local_a) = a.to_chunk_and_local();
    let (chunk_b, local_b) = b.to_chunk_and_local();

    // Get slot indices for both chunks
    let Some(idx_a) = self.pool.index_for(chunk_a) else {
      return false;
    };
    let Some(idx_b) = self.pool.index_for(chunk_b) else {
      return false;
    };

    // Check both are seeded
    if !self.pool.get(idx_a).is_seeded() || !self.pool.get(idx_b).is_seeded() {
      return false;
    }

    if chunk_a == chunk_b {
      // Same chunk - simple swap
      let slot = self.pool.get_mut(idx_a);
      let (la, lb) = (
        (local_a.x as u32, local_a.y as u32),
        (local_b.x as u32, local_b.y as u32),
      );
      let pixel_a = slot.chunk.pixels[la];
      let pixel_b = slot.chunk.pixels[lb];
      slot.chunk.pixels[la] = pixel_b;
      slot.chunk.pixels[lb] = pixel_a;
      slot.dirty = true;
      slot.modified = true;
      slot.persisted = false;
    } else {
      // Different chunks - need to swap across
      let (slot_a, slot_b) = self.pool.get_two_mut(idx_a, idx_b);

      let la = (local_a.x as u32, local_a.y as u32);
      let lb = (local_b.x as u32, local_b.y as u32);
      std::mem::swap(&mut slot_a.chunk.pixels[la], &mut slot_b.chunk.pixels[lb]);
      slot_a.dirty = true;
      slot_a.modified = true;
      slot_a.persisted = false;
      slot_b.dirty = true;
      slot_b.modified = true;
      slot_b.persisted = false;
    }

    true
  }

  /// Sets the pixel at the given world position.
  ///
  /// Returns true if the pixel was set, false if the chunk is not loaded
  /// or not yet seeded.
  ///
  /// The `debug_gizmos` parameter emits visual debug overlays when the
  /// `visual-debug` feature is enabled. Pass `()` when disabled.
  pub fn set_pixel(&mut self, pos: WorldPos, pixel: Pixel, debug_gizmos: DebugGizmos<'_>) -> bool {
    let (chunk_pos, local_pos) = pos.to_chunk_and_local();
    let Some(idx) = self.pool.index_for(chunk_pos) else {
      return false;
    };
    let slot = self.pool.get_mut(idx);
    if !slot.is_seeded() {
      return false;
    }
    slot.chunk.pixels[(local_pos.x as u32, local_pos.y as u32)] = pixel;
    let was_clean = !slot.dirty;
    slot.dirty = true;
    slot.modified = true;
    slot.persisted = false; // Needs saving again

    // Emit chunk gizmo if this is the first modification
    if was_clean {
      debug_shim::emit_chunk(debug_gizmos, chunk_pos);
    }

    true
  }

  /// Marks a chunk as needing GPU upload.
  pub fn mark_dirty(&mut self, pos: crate::pixel_world::coords::ChunkPos) {
    if let Some(idx) = self.pool.index_for(pos) {
      self.pool.get_mut(idx).dirty = true;
    }
  }

  /// Returns the heat value at the given world position.
  ///
  /// Maps the pixel position to its heat cell (4x4 downsampling).
  /// Returns None if the chunk is not loaded or not yet seeded.
  pub fn get_heat_at(&self, pos: WorldPos) -> Option<u8> {
    let (chunk_pos, local_pos) = pos.to_chunk_and_local();
    let idx = self.pool.index_for(chunk_pos)?;
    let slot = self.pool.get(idx);
    if !slot.is_seeded() {
      return None;
    }
    let hx = local_pos.x as u32 / HEAT_CELL_SIZE;
    let hy = local_pos.y as u32 / HEAT_CELL_SIZE;
    Some(slot.chunk.heat_cell(hx, hy))
  }

  /// Sets the heat value at the given world position's heat cell.
  ///
  /// Returns true if the heat was set, false if the chunk is not loaded.
  /// Also marks the heat tile dirty so propagation will process it.
  pub fn set_heat_at(&mut self, pos: WorldPos, heat: u8) -> bool {
    let (chunk_pos, local_pos) = pos.to_chunk_and_local();
    let Some(idx) = self.pool.index_for(chunk_pos) else {
      return false;
    };
    let slot = self.pool.get_mut(idx);
    if !slot.is_seeded() {
      return false;
    }
    let hx = local_pos.x as u32 / HEAT_CELL_SIZE;
    let hy = local_pos.y as u32 / HEAT_CELL_SIZE;
    *slot.chunk.heat_cell_mut(hx, hy) = heat;

    // Mark heat tile dirty so propagation will process it
    if heat > 0 {
      slot.chunk.heat_dirty.mark_dirty(hx, hy);
    }

    true
  }

  /// Marks a world position as simulation-dirty.
  ///
  /// This expands the tile dirty rect so the CA simulation will process
  /// the pixel on the next tick. Use this when placing material that needs
  /// to participate in simulation (e.g., displaced water).
  pub fn mark_pixel_sim_dirty(&mut self, pos: WorldPos) {
    let (chunk_pos, local_pos) = pos.to_chunk_and_local();
    let Some(idx) = self.pool.index_for(chunk_pos) else {
      return;
    };
    let slot = self.pool.get_mut(idx);
    if !slot.is_seeded() {
      return;
    }
    slot
      .chunk
      .mark_pixel_dirty(local_pos.x as u32, local_pos.y as u32);
  }
}
