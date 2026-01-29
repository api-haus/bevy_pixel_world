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
mod bomb;
mod collider;
mod displacement;
mod loader;
mod readback;
mod spawn;
mod split;

use bevy::prelude::*;
pub use blit::{LastBlitTransform, WrittenPixel, update_pixel_bodies};
pub(crate) use blit::{compute_transformed_aabb, compute_world_aabb};
pub use bomb::{Bomb, BombInitialState, check_bomb_damage, init_bomb_state, process_detonations};
pub use collider::generate_collider;
pub use displacement::DisplacementState;
pub use loader::PixelBodyLoader;
pub use readback::{
  apply_readback_changes, detect_external_erasure, readback_pixel_bodies, sync_simulation_to_bodies,
};
pub use spawn::{
  PendingPixelBody, PixelBodyIdGenerator, SpawnPixelBody, SpawnPixelBodyFromImage,
  finalize_pending_pixel_bodies,
};
pub use split::split_pixel_bodies;

/// Stable identifier for pixel bodies across sessions.
///
/// Each pixel body gets a unique ID when spawned. This ID persists across
/// save/load cycles and is used to track bodies for persistence.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PixelBodyId(pub u64);

impl PixelBodyId {
  /// Creates a new pixel body ID.
  pub fn new(id: u64) -> Self {
    Self(id)
  }

  /// Returns the raw ID value.
  pub fn value(&self) -> u64 {
    self.0
  }
}

/// Marker for pixel bodies that should persist with chunks.
///
/// When a chunk unloads, pixel bodies with this component are saved to disk.
/// They are restored when the chunk loads again.
#[derive(Component, Default)]
pub struct Persistable;

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

  /// Returns the linear index for local coordinates, or None if out of
  /// bounds.
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

  /// Maps a world-space point to local pixel coordinates if it hits a solid
  /// pixel.
  ///
  /// Applies the inverse transform, subtracts the origin, performs bounds
  /// checking, and verifies the pixel is solid. Returns `None` if the point
  /// is outside the body or lands on a non-solid pixel.
  pub fn world_to_solid_local(
    &self,
    world_point: Vec3,
    inverse: &bevy::math::Affine3A,
  ) -> Option<(u32, u32)> {
    let local_point = inverse.transform_point3(world_point);
    let local_x = (local_point.x - self.origin.x as f32).floor() as i32;
    let local_y = (local_point.y - self.origin.y as f32).floor() as i32;

    if local_x < 0
      || local_x >= self.width() as i32
      || local_y < 0
      || local_y >= self.height() as i32
    {
      return None;
    }

    let (lx, ly) = (local_x as u32, local_y as u32);
    if !self.is_solid(lx, ly) {
      return None;
    }

    Some((lx, ly))
  }
}

/// Marker component indicating this pixel body needs its collider regenerated.
#[derive(Component)]
pub struct NeedsColliderRegen;

/// Marker component indicating the shape mask was modified by readback.
///
/// Added by `readback_pixel_bodies` when pixels are destroyed. The split system
/// uses this to check for fragmentation and handle entity splitting.
#[derive(Component)]
pub struct ShapeMaskModified;
