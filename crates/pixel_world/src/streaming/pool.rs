//! Chunk memory pool for zero-allocation streaming.
//!
//! Pre-allocates [`POOL_SIZE`] chunks to avoid runtime allocations
//! when chunks enter or leave the streaming window.
//!
//! See `docs/architecture/chunk-pooling.md` for the design rationale.

use crate::coords::{CHUNK_SIZE, POOL_SIZE};
use crate::Chunk;

/// A slot in the chunk pool.
struct PoolSlot {
  chunk: Chunk,
  in_use: bool,
}

/// Handle to an acquired chunk in the pool.
#[derive(Clone, Copy, Debug)]
pub struct PoolHandle(pub(crate) usize);

/// Pre-allocated pool of chunks.
///
/// Provides O(n) acquire and O(1) release operations.
/// With a small pool size (24 chunks), linear search is fast enough.
pub struct ChunkPool {
  slots: Vec<PoolSlot>,
}

impl ChunkPool {
  /// Creates a new pool with [`POOL_SIZE`] pre-allocated chunks.
  pub fn new() -> Self {
    let slots = (0..POOL_SIZE)
      .map(|_| PoolSlot {
        chunk: Chunk::new(CHUNK_SIZE, CHUNK_SIZE),
        in_use: false,
      })
      .collect();

    Self { slots }
  }

  /// Acquires a free chunk from the pool.
  ///
  /// Returns `None` if all chunks are in use.
  pub fn acquire(&mut self) -> Option<PoolHandle> {
    for (i, slot) in self.slots.iter_mut().enumerate() {
      if !slot.in_use {
        slot.in_use = true;
        return Some(PoolHandle(i));
      }
    }
    None
  }

  /// Releases a chunk back to the pool.
  ///
  /// # Panics
  /// Panics if the handle is invalid or already released.
  pub fn release(&mut self, handle: PoolHandle) {
    let slot = &mut self.slots[handle.0];
    assert!(slot.in_use, "double release of pool handle");
    slot.chunk.clear_pos();
    slot.in_use = false;
  }

  /// Returns a reference to the chunk for the given handle.
  ///
  /// # Panics
  /// Panics if the handle is invalid.
  pub fn get(&self, handle: PoolHandle) -> &Chunk {
    &self.slots[handle.0].chunk
  }

  /// Returns a mutable reference to the chunk for the given handle.
  ///
  /// # Panics
  /// Panics if the handle is invalid.
  pub fn get_mut(&mut self, handle: PoolHandle) -> &mut Chunk {
    &mut self.slots[handle.0].chunk
  }
}

impl Default for ChunkPool {
  fn default() -> Self {
    Self::new()
  }
}
