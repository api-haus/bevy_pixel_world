//! Simulation pixel format.

use crate::coords::{ColorIndex, MaterialId};

/// Pixel flag bit positions.
pub mod flags {
  /// Pixel is active and needs simulation this tick.
  pub const DIRTY: u8 = 0b0000_0001;
  /// Pixel's material is solid or powder (not liquid/gas).
  pub const SOLID: u8 = 0b0000_0010;
  /// Pixel has downward momentum.
  pub const FALLING: u8 = 0b0000_0100;
}

/// Simulation pixel - 4 bytes for cache efficiency.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Pixel {
  pub material: MaterialId,
  pub color: ColorIndex,
  pub damage: u8,
  pub flags: u8,
}

impl Pixel {
  pub const AIR: Self = Self {
    material: MaterialId(0),
    color: ColorIndex(0),
    damage: 0,
    flags: 0,
  };

  pub fn new(material: MaterialId, color: ColorIndex) -> Self {
    Self {
      material,
      color,
      damage: 0,
      flags: 0,
    }
  }

  /// Returns true if the pixel is air (empty space).
  #[inline]
  pub fn is_air(&self) -> bool {
    self.material.0 == 0
  }

  /// Returns true if the given flag is set.
  #[inline]
  pub fn has_flag(&self, flag: u8) -> bool {
    self.flags & flag != 0
  }

  /// Sets the given flag.
  #[inline]
  pub fn set_flag(&mut self, flag: u8) {
    self.flags |= flag;
  }

  /// Clears the given flag.
  #[inline]
  pub fn clear_flag(&mut self, flag: u8) {
    self.flags &= !flag;
  }
}

pub type PixelSurface = crate::primitives::Surface<Pixel>;
