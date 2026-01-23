//! RGBA pixel type for GPU rendering.

/// RGBA pixel with 8 bits per channel.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Rgba {
  pub r: u8,
  pub g: u8,
  pub b: u8,
  pub a: u8,
}

impl Rgba {
  /// Creates a new RGBA pixel.
  #[inline]
  pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
    Self { r, g, b, a }
  }

  /// Creates an opaque RGB pixel (alpha = 255).
  #[inline]
  pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
    Self { r, g, b, a: 255 }
  }

  /// Transparent black.
  pub const TRANSPARENT: Self = Self::new(0, 0, 0, 0);

  /// Opaque black.
  pub const BLACK: Self = Self::rgb(0, 0, 0);

  /// Opaque white.
  pub const WHITE: Self = Self::rgb(255, 255, 255);
}
