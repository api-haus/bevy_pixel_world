//! Blittable pixel buffer for 2D rendering.
//!
//! A [`Surface`] is a generic 2D buffer that can hold any element type.
//! The primary use case is [`RgbaSurface`] for GPU-uploadable pixel data.
//!
//! See `docs/architecture/pixel-format.md` for the 4-byte pixel structure.
//!
//! # Coordinate System
//!
//! Surfaces use a Y+ up coordinate system consistent with world coordinates:
//! - **X+** is to the right (east)
//! - **Y+** is upward (toward sky)
//! - **(0, 0)** is the bottom-left corner
//!
//! Data is stored in row-major order where row 0 is the bottom of the surface.

use std::ops::{Index, IndexMut};

use crate::render::Rgba;

// Ensure Rgba has the correct size for as_bytes() to work correctly.
// palette::Srgba<u8> should be 4 bytes (u8 x 4) with #[repr(C)].
const _: () = assert!(std::mem::size_of::<Rgba>() == 4);

/// A 2D buffer of elements.
///
/// Data is stored in row-major order (y * width + x).
pub struct Surface<T> {
  data: Box<[T]>,
  width: u32,
  height: u32,
}

impl<T: Clone + Default> Surface<T> {
  /// Creates a new surface filled with the default value.
  pub fn new(width: u32, height: u32) -> Self {
    let len = (width as usize) * (height as usize);
    Self {
      data: vec![T::default(); len].into_boxed_slice(),
      width,
      height,
    }
  }

  /// Creates a new surface filled with the given value.
  pub fn filled(width: u32, height: u32, value: T) -> Self {
    let len = (width as usize) * (height as usize);
    Self {
      data: vec![value; len].into_boxed_slice(),
      width,
      height,
    }
  }
}

impl<T> Surface<T> {
  /// Returns the width of the surface.
  #[inline]
  pub fn width(&self) -> u32 {
    self.width
  }

  /// Returns the height of the surface.
  #[inline]
  pub fn height(&self) -> u32 {
    self.height
  }

  /// Converts (x, y) to a linear index, or `None` if out of bounds.
  #[inline]
  fn index_of(&self, x: u32, y: u32) -> Option<usize> {
    if x < self.width && y < self.height {
      Some((y as usize) * (self.width as usize) + (x as usize))
    } else {
      None
    }
  }

  /// Returns a reference to the element at (x, y), or `None` if out of bounds.
  #[inline]
  pub fn get(&self, x: u32, y: u32) -> Option<&T> {
    self.index_of(x, y).map(|i| &self.data[i])
  }

  /// Returns a mutable reference to the element at (x, y), or `None` if out of
  /// bounds.
  #[inline]
  pub fn get_mut(&mut self, x: u32, y: u32) -> Option<&mut T> {
    self.index_of(x, y).map(|i| &mut self.data[i])
  }

  /// Sets the element at (x, y). Returns `true` if successful, `false` if out
  /// of bounds.
  #[inline]
  pub fn set(&mut self, x: u32, y: u32, value: T) -> bool {
    if let Some(i) = self.index_of(x, y) {
      self.data[i] = value;
      true
    } else {
      false
    }
  }

  /// Returns the raw data as a byte slice (for GPU upload).
  ///
  /// # Safety
  /// This reinterprets the data as bytes. Only safe when `T` is `#[repr(C)]`
  /// and has no padding.
  #[inline]
  pub fn as_bytes(&self) -> &[u8] {
    let ptr = self.data.as_ptr() as *const u8;
    let len = self.data.len() * std::mem::size_of::<T>();
    // SAFETY: Surface data is contiguous and T is expected to be repr(C)
    unsafe { std::slice::from_raw_parts(ptr, len) }
  }

  /// Returns a slice of the underlying data.
  #[inline]
  pub fn as_slice(&self) -> &[T] {
    &self.data
  }

  /// Returns a mutable slice of the underlying data.
  #[inline]
  pub fn as_slice_mut(&mut self) -> &mut [T] {
    &mut self.data
  }

  /// Fills the entire surface with the given value.
  #[inline]
  pub fn fill(&mut self, value: T)
  where
    T: Clone,
  {
    self.data.fill(value);
  }
}

impl<T> Index<(u32, u32)> for Surface<T> {
  type Output = T;

  #[inline]
  fn index(&self, (x, y): (u32, u32)) -> &Self::Output {
    let i = (y as usize) * (self.width as usize) + (x as usize);
    &self.data[i]
  }
}

impl<T> IndexMut<(u32, u32)> for Surface<T> {
  #[inline]
  fn index_mut(&mut self, (x, y): (u32, u32)) -> &mut Self::Output {
    let i = (y as usize) * (self.width as usize) + (x as usize);
    &mut self.data[i]
  }
}

impl Surface<crate::pixel::Pixel> {
  /// Returns pixel data as bytes with `PIXEL_BODY` pixels replaced by
  /// `Pixel::VOID`.
  ///
  /// Used when saving chunk data to avoid baking body pixels into persistent
  /// storage. Bodies are saved separately and re-blitted on load.
  pub fn bytes_without_body_pixels(&self) -> Vec<u8> {
    use crate::pixel::{Pixel, PixelFlags};

    let mut bytes = self.as_bytes().to_vec();
    let pixel_size = std::mem::size_of::<Pixel>();
    let void_bytes: [u8; 4] = unsafe { std::mem::transmute(Pixel::VOID) };

    for (i, pixel) in self.data.iter().enumerate() {
      if pixel.flags.contains(PixelFlags::PIXEL_BODY) {
        let offset = i * pixel_size;
        bytes[offset..offset + pixel_size].copy_from_slice(&void_bytes);
      }
    }

    bytes
  }
}

/// A surface containing RGBA pixels, suitable for GPU upload.
///
/// Used for GPU upload and basic examples only. Simulation uses
/// [`PixelSurface`].
pub type RgbaSurface = Surface<Rgba>;
