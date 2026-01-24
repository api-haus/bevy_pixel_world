//! Pixel bodies - dynamic physics objects with pixel content.
//!
//! Pixel bodies are rigid bodies whose visual representation consists of
//! individual pixels that participate in cellular automata simulation.
//!
//! # Simulation Cycle
//!
//! Each tick:
//! 1. **Blit**: Write object pixels to Canvas at world-transformed positions
//! 2. **CA Simulation**: Standard cellular automata passes
//! 3. **Readback**: Map CA changes back to object state (Phase 2)
//! 4. **Clear**: Remove object pixels from Canvas
//! 5. **Physics Step**: Rapier/Avian moves bodies
//! 6. **Transform Update**: New positions applied
//!
//! # Spawning
//!
//! Use [`SpawnPixelBody`] to spawn pixel bodies from image assets:
//!
//! ```ignore
//! commands.queue(SpawnPixelBody::new(
//!     "sprites/crate.png",
//!     material_ids::WOOD,
//!     Vec2::new(100.0, 200.0),
//! ));
//! ```

mod blit;
mod collider;
mod loader;
mod spawn;

use bevy::prelude::*;
pub use blit::{BlittedTransform, blit_pixel_bodies, clear_pixel_bodies};
pub use collider::generate_collider;
pub use loader::PixelBodyLoader;
pub use spawn::{
  PendingPixelBody, SpawnPixelBody, SpawnPixelBodyFromImage, finalize_pending_pixel_bodies,
};

use crate::pixel::Pixel;
use crate::primitives::Surface;

/// A physics object composed of pixels.
///
/// The surface buffer contains object-local pixel data. The shape mask tracks
/// which pixels belong to the object (vs void). Transform determines world
/// position.
#[derive(Component)]
pub struct PixelBody {
  /// Object-local pixel buffer.
  pub surface: Surface<Pixel>,
  /// Which pixels belong to the object (row-major, true = solid).
  pub shape_mask: Vec<bool>,
  /// Offset from entity transform origin to pixel grid center.
  pub origin: IVec2,
}

impl PixelBody {
  /// Creates a new pixel body with the given dimensions.
  ///
  /// The origin is set to center the pixel grid on the entity origin.
  pub fn new(width: u32, height: u32) -> Self {
    let len = (width as usize) * (height as usize);
    Self {
      surface: Surface::new(width, height),
      shape_mask: vec![false; len],
      origin: IVec2::new(-(width as i32) / 2, -(height as i32) / 2),
    }
  }

  /// Returns the width of the pixel grid.
  #[inline]
  pub fn width(&self) -> u32 {
    self.surface.width()
  }

  /// Returns the height of the pixel grid.
  #[inline]
  pub fn height(&self) -> u32 {
    self.surface.height()
  }

  /// Returns the linear index for local coordinates, or None if out of bounds.
  #[inline]
  fn index_of(&self, x: u32, y: u32) -> Option<usize> {
    if x < self.width() && y < self.height() {
      Some((y as usize) * (self.width() as usize) + (x as usize))
    } else {
      None
    }
  }

  /// Returns whether the pixel at local (x, y) belongs to the object.
  #[inline]
  pub fn is_solid(&self, x: u32, y: u32) -> bool {
    self
      .index_of(x, y)
      .map(|i| self.shape_mask[i])
      .unwrap_or(false)
  }

  /// Sets whether the pixel at local (x, y) belongs to the object.
  #[inline]
  pub fn set_solid(&mut self, x: u32, y: u32, solid: bool) {
    if let Some(i) = self.index_of(x, y) {
      self.shape_mask[i] = solid;
    }
  }

  /// Returns the pixel at local (x, y).
  #[inline]
  pub fn get_pixel(&self, x: u32, y: u32) -> Option<&Pixel> {
    self.surface.get(x, y)
  }

  /// Sets the pixel at local (x, y) and marks it as solid in the shape mask.
  #[inline]
  pub fn set_pixel(&mut self, x: u32, y: u32, pixel: Pixel) {
    if self.surface.set(x, y, pixel) {
      self.set_solid(x, y, !pixel.is_void());
    }
  }

  /// Returns the number of solid pixels in the shape mask.
  pub fn solid_count(&self) -> usize {
    self.shape_mask.iter().filter(|&&s| s).count()
  }

  /// Returns true if the shape mask is entirely empty.
  pub fn is_empty(&self) -> bool {
    !self.shape_mask.iter().any(|&s| s)
  }
}

/// Marker component indicating this pixel body needs its collider regenerated.
#[derive(Component)]
pub struct NeedsColliderRegen;
