//! Simulation pixel format.

use crate::coords::{ColorIndex, MaterialId};

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
}

pub type PixelSurface = crate::primitives::Surface<Pixel>;
