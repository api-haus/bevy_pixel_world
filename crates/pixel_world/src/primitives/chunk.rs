//! Chunk - a spatial unit containing pixel data.
//!
//! A chunk is the basic unit of the world, containing a surface of pixels.
//!
//! See `docs/architecture/spatial-hierarchy.md` for the four-level spatial
//! organization. See `docs/architecture/chunk-pooling.md` for the pooling
//! lifecycle.

use crate::coords::ChunkPos;
use crate::pixel::PixelSurface;

/// A chunk of the world containing pixel data.
pub struct Chunk {
  /// Simulation data (material, color, damage, flags).
  pub pixels: PixelSurface,
  /// World position of this chunk. `None` when in the pool, `Some` when
  /// assigned.
  pos: Option<ChunkPos>,
}

impl Chunk {
  /// Creates a new chunk with the given dimensions.
  pub fn new(width: u32, height: u32) -> Self {
    Self {
      pixels: PixelSurface::new(width, height),
      pos: None,
    }
  }

  /// Returns the world position of this chunk, if assigned.
  pub fn pos(&self) -> Option<ChunkPos> {
    self.pos
  }

  /// Sets the world position of this chunk.
  pub fn set_pos(&mut self, pos: ChunkPos) {
    self.pos = Some(pos);
  }

  /// Clears the world position (called when chunk returns to pool).
  pub fn clear_pos(&mut self) {
    self.pos = None;
  }
}
