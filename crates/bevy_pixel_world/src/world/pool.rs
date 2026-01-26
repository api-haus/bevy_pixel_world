//! Chunk pool for managing slot storage.
//!
//! This module encapsulates all slot management including the unsafe
//! `collect_seeded_mut` method for parallel chunk access.

use std::collections::HashMap;

use super::slot::{ChunkSlot, SlotIndex};
use crate::coords::{ChunkPos, POOL_SIZE};
use crate::primitives::Chunk;

/// Fixed-size pool of chunk slots.
///
/// Manages slot allocation and provides indexed access to chunks.
/// Encapsulates the active chunk position mapping and unsafe pointer
/// manipulation for parallel access.
pub(crate) struct ChunkPool {
  /// Fixed array of chunk slots (pre-allocated memory).
  slots: Vec<ChunkSlot>,
  /// Maps active chunk positions to slot indices.
  active: HashMap<ChunkPos, SlotIndex>,
}

impl ChunkPool {
  /// Creates a new chunk pool with pre-allocated slots.
  pub fn new() -> Self {
    let slots = (0..POOL_SIZE).map(|_| ChunkSlot::new()).collect();
    Self {
      slots,
      active: HashMap::new(),
    }
  }

  /// Acquires a free slot from the pool.
  ///
  /// Returns None if all slots are in use.
  pub fn acquire(&mut self) -> Option<SlotIndex> {
    for (i, slot) in self.slots.iter_mut().enumerate() {
      if slot.is_free() {
        return Some(SlotIndex(i));
      }
    }
    None
  }

  /// Gets a reference to a slot by index.
  #[inline]
  pub fn get(&self, index: SlotIndex) -> &ChunkSlot {
    &self.slots[index.0]
  }

  /// Gets a mutable reference to a slot by index.
  #[inline]
  pub fn get_mut(&mut self, index: SlotIndex) -> &mut ChunkSlot {
    &mut self.slots[index.0]
  }

  /// Gets the slot index for an active chunk position.
  pub fn index_for(&self, pos: ChunkPos) -> Option<SlotIndex> {
    self.active.get(&pos).copied()
  }

  /// Returns an iterator over active chunk positions and their slot indices.
  pub fn iter_active(&self) -> impl Iterator<Item = (ChunkPos, SlotIndex)> + '_ {
    self.active.iter().map(|(&pos, &idx)| (pos, idx))
  }

  /// Returns the number of active chunks.
  pub fn active_count(&self) -> usize {
    self.active.len()
  }

  /// Activates a slot for the given position.
  ///
  /// The slot must already be acquired and initialized.
  pub fn activate(&mut self, pos: ChunkPos, idx: SlotIndex) {
    self.active.insert(pos, idx);
  }

  /// Deactivates a chunk position, returning the slot index if present.
  pub fn deactivate(&mut self, pos: &ChunkPos) -> Option<SlotIndex> {
    self.active.remove(pos)
  }

  /// Returns mutable references to two different slots.
  ///
  /// Panics if idx_a == idx_b.
  pub fn get_two_mut(
    &mut self,
    idx_a: SlotIndex,
    idx_b: SlotIndex,
  ) -> (&mut ChunkSlot, &mut ChunkSlot) {
    assert_ne!(idx_a.0, idx_b.0, "Cannot get two mutable refs to same slot");
    if idx_a.0 < idx_b.0 {
      let (left, right) = self.slots.split_at_mut(idx_b.0);
      (&mut left[idx_a.0], &mut right[0])
    } else {
      let (left, right) = self.slots.split_at_mut(idx_a.0);
      (&mut right[0], &mut left[idx_b.0])
    }
  }

  /// Collects mutable references to all seeded chunks for parallel access.
  ///
  /// # Safety
  /// This method uses raw pointers to work around the borrow checker.
  /// It is safe because:
  /// - Each slot appears at most once in `self.active` (unique SlotIndex
  ///   values)
  /// - Slots are stored in a Vec with distinct indices
  /// - The resulting mutable references are non-overlapping
  pub fn collect_seeded_mut(&mut self) -> HashMap<ChunkPos, &mut Chunk> {
    let seeded_positions: Vec<_> = self
      .active
      .iter()
      .filter_map(|(&pos, &idx)| {
        if self.slots[idx.0].is_seeded() {
          Some((pos, idx))
        } else {
          None
        }
      })
      .collect();

    let mut chunks: HashMap<ChunkPos, &mut Chunk> = HashMap::new();
    for (pos, idx) in seeded_positions {
      // SAFETY: seeded_positions contains unique SlotIndex values.
      let chunk = &mut self.slots[idx.0].chunk;
      let chunk_ptr = chunk as *mut Chunk;
      chunks.insert(pos, unsafe { &mut *chunk_ptr });
    }
    chunks
  }
}

impl Default for ChunkPool {
  fn default() -> Self {
    Self::new()
  }
}
