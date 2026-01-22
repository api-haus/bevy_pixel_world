//! Chunk - a spatial unit containing pixel data.
//!
//! A chunk is the basic unit of the world, containing a surface of pixels.

use super::surface::RgbaSurface;

/// A chunk of the world containing pixel data.
pub struct Chunk {
  /// The pixel data for this chunk.
  pub pixels: RgbaSurface,
}

impl Chunk {
  /// Creates a new chunk with the given dimensions.
  pub fn new(width: u32, height: u32) -> Self {
    Self {
      pixels: RgbaSurface::new(width, height),
    }
  }
}
