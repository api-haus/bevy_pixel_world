//! Canvas abstraction for world-coordinate pixel operations.
//!
//! The [`Canvas`] provides a high-level API for reading and writing pixels
//! using world coordinates, automatically handling chunk boundaries.

pub mod blitter;

use std::collections::HashMap;
use std::sync::Mutex;

pub use blitter::Phase;

use crate::coords::{ChunkPos, WorldFragment, WorldPos, WorldRect};
use crate::pixel::Pixel;
use crate::primitives::Chunk;

/// World-coordinate canvas for multi-chunk pixel operations.
///
/// Wraps a set of chunks and provides methods for reading/writing pixels
/// using world coordinates. The `blit` method uses parallel execution with
/// 2x2 checkerboard scheduling for thread-safe concurrent writes.
pub struct Canvas<'a> {
  chunks: HashMap<ChunkPos, &'a mut Chunk>,
}

impl<'a> Canvas<'a> {
  /// Creates a new canvas from borrowed chunks.
  pub fn new(chunks: HashMap<ChunkPos, &'a mut Chunk>) -> Self {
    Self { chunks }
  }

  /// Returns a reference to the pixel at the given world position.
  ///
  /// Returns `None` if the position is not in any loaded chunk.
  pub fn get_pixel(&self, pos: WorldPos) -> Option<&Pixel> {
    let (chunk_pos, local_pos) = pos.to_chunk_and_local();
    self
      .chunks
      .get(&chunk_pos)
      .map(|chunk| &chunk.pixels[(local_pos.0 as u32, local_pos.1 as u32)])
  }

  /// Sets the pixel at the given world position.
  ///
  /// Returns `true` if the pixel was set, `false` if the position is not
  /// in any loaded chunk.
  pub fn set_pixel(&mut self, pos: WorldPos, pixel: Pixel) -> bool {
    let (chunk_pos, local_pos) = pos.to_chunk_and_local();
    if let Some(chunk) = self.chunks.get_mut(&chunk_pos) {
      chunk.pixels[(local_pos.0 as u32, local_pos.1 as u32)] = pixel;
      true
    } else {
      false
    }
  }

  /// Blits pixels to the canvas using a shader function.
  ///
  /// For each pixel in `rect`, calls `f(fragment)` where fragment contains
  /// absolute world coordinates and normalized UV coordinates within the rect.
  /// If `f` returns `Some(pixel)`, that pixel is written; if `None`, the pixel
  /// is left unchanged.
  ///
  /// Uses 2x2 checkerboard scheduling for parallel execution. Tiles are grouped
  /// into four phases (A, B, C, D) based on position modulo 2. Tiles in the
  /// same phase are never adjacent, allowing safe concurrent writes.
  ///
  /// Returns the list of chunk positions that were modified.
  pub fn blit<F>(self, rect: WorldRect, f: F) -> Vec<ChunkPos>
  where
    F: Fn(WorldFragment) -> Option<Pixel> + Sync,
  {
    let locked = blitter::LockedChunks::new(self.chunks);
    let dirty_chunks = Mutex::new(Vec::new());

    blitter::parallel_blit(&locked, rect, f, &dirty_chunks);

    dirty_chunks.into_inner().unwrap_or_default()
  }
}
