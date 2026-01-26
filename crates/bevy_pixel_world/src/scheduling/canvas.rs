//! Canvas - unified drawing surface for cross-chunk pixel operations.
//!
//! The [`Canvas`] provides a single coordinate space for pixel operations
//! across chunk boundaries. It uses interior mutability that is sound only
//! when used with 2x2 checkerboard scheduling.

use std::cell::UnsafeCell;
use std::collections::HashMap;

use crate::coords::ChunkPos;
use crate::primitives::Chunk;

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
  chunks: HashMap<ChunkPos, UnsafeCell<*mut Chunk>>,
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
      .map(|(pos, chunk)| (pos, UnsafeCell::new(chunk as *mut Chunk)))
      .collect();
    Self {
      chunks: ptrs,
      _marker: std::marker::PhantomData,
    }
  }

  /// Gets a chunk reference for reading.
  #[inline]
  pub fn get(&self, pos: ChunkPos) -> Option<&Chunk> {
    self.chunks.get(&pos).map(|cell| unsafe { &**cell.get() })
  }

  /// Gets a mutable chunk reference for writing.
  ///
  /// # Safety
  /// Interior mutability is sound due to 2x2 checkerboard scheduling, which
  /// guarantees tiles in the same phase never access overlapping pixels.
  #[inline]
  #[allow(clippy::mut_from_ref)]
  pub fn get_mut(&self, pos: ChunkPos) -> Option<&mut Chunk> {
    self
      .chunks
      .get(&pos)
      .map(|cell| unsafe { &mut **cell.get() })
  }

  /// Gets mutable references to two different chunks.
  ///
  /// Returns None if either chunk is not found. Panics if `a == b`
  /// (use `get_mut` for same-chunk access).
  ///
  /// # Safety
  /// Interior mutability is sound because:
  /// - The positions are guaranteed to be different (distinct memory)
  /// - Checkerboard scheduling guarantees no overlapping pixel access
  #[inline]
  #[allow(clippy::mut_from_ref)]
  pub fn get_two_mut(&self, a: ChunkPos, b: ChunkPos) -> Option<(&mut Chunk, &mut Chunk)> {
    debug_assert_ne!(a, b, "get_two_mut requires different chunk positions");
    let cell_a = self.chunks.get(&a)?;
    let cell_b = self.chunks.get(&b)?;
    // SAFETY: a != b guarantees these are distinct memory locations.
    // Checkerboard scheduling guarantees no overlapping pixel access.
    Some(unsafe { (&mut **cell_a.get(), &mut **cell_b.get()) })
  }
}
