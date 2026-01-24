//! Simulation pixel format.

use crate::coords::{ColorIndex, MaterialId};

bitflags::bitflags! {
  /// Pixel state flags.
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
  pub struct PixelFlags: u8 {
    /// Pixel is active and needs simulation this tick.
    const DIRTY = 0b0000_0001;
    /// Pixel's material is solid or powder (not liquid/gas).
    const SOLID = 0b0000_0010;
    /// Pixel has downward momentum.
    const FALLING = 0b0000_0100;
    /// Pixel is burning (reserved for future use).
    const BURNING = 0b0000_1000;
    /// Pixel is wet (reserved for future use).
    const WET = 0b0001_0000;
    /// Pixel belongs to a pixel body (excluded from terrain collision).
    const PIXEL_BODY = 0b0010_0000;
  }
}

/// Simulation pixel - 4 bytes for cache efficiency.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pixel {
  pub material: MaterialId,
  pub color: ColorIndex,
  pub damage: u8,
  pub flags: PixelFlags,
}

impl Default for Pixel {
  fn default() -> Self {
    Self::VOID
  }
}

impl Pixel {
  pub const VOID: Self = Self {
    material: MaterialId(0),
    color: ColorIndex(0),
    damage: 0,
    flags: PixelFlags::empty(),
  };

  pub fn new(material: MaterialId, color: ColorIndex) -> Self {
    Self {
      material,
      color,
      damage: 0,
      flags: PixelFlags::empty(),
    }
  }

  /// Returns true if the pixel is void (empty space).
  #[inline]
  pub fn is_void(&self) -> bool {
    self.material.0 == 0
  }

  /// Returns the flags as a raw u8 for serialization.
  #[inline]
  pub fn flags_bits(&self) -> u8 {
    self.flags.bits()
  }

  /// Sets flags from raw u8 bits (for deserialization).
  #[inline]
  pub fn set_flags_bits(&mut self, bits: u8) {
    self.flags = PixelFlags::from_bits_truncate(bits);
  }
}

pub type PixelSurface = crate::primitives::Surface<Pixel>;
