//! Blittable pixel buffer for 2D rendering.
//!
//! A [`Surface`] is a generic 2D buffer that can hold any element type.
//! The primary use case is [`RgbaSurface`] for GPU-uploadable pixel data.

use std::ops::{Index, IndexMut};

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

    /// Returns a mutable reference to the element at (x, y), or `None` if out of bounds.
    #[inline]
    pub fn get_mut(&mut self, x: u32, y: u32) -> Option<&mut T> {
        self.index_of(x, y).map(|i| &mut self.data[i])
    }

    /// Sets the element at (x, y). Returns `true` if successful, `false` if out of bounds.
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

/// A surface containing RGBA pixels, suitable for GPU upload.
pub type RgbaSurface = Surface<Rgba>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_index_calculation() {
        let surface: Surface<u8> = Surface::filled(10, 5, 0);
        // Verify index_of returns y * width + x
        assert_eq!(surface.index_of(0, 0), Some(0));
        assert_eq!(surface.index_of(1, 0), Some(1));
        assert_eq!(surface.index_of(0, 1), Some(10));
        assert_eq!(surface.index_of(3, 2), Some(23)); // 2 * 10 + 3
        assert_eq!(surface.index_of(9, 4), Some(49)); // 4 * 10 + 9 (last element)
    }

    #[test]
    fn surface_out_of_bounds() {
        let mut surface: Surface<u8> = Surface::filled(10, 5, 42);

        // get returns None for out of bounds
        assert!(surface.get(10, 0).is_none());
        assert!(surface.get(0, 5).is_none());
        assert!(surface.get(100, 100).is_none());

        // set returns false for out of bounds
        assert!(!surface.set(10, 0, 99));
        assert!(!surface.set(0, 5, 99));
        assert!(!surface.set(100, 100, 99));

        // Valid coordinates work
        assert_eq!(surface.get(0, 0), Some(&42));
        assert!(surface.set(0, 0, 99));
        assert_eq!(surface.get(0, 0), Some(&99));
    }

    #[test]
    fn rgba_surface_as_bytes() {
        let mut surface = RgbaSurface::new(2, 2);
        surface.set(0, 0, Rgba::rgb(255, 0, 0));
        surface.set(1, 0, Rgba::rgb(0, 255, 0));
        surface.set(0, 1, Rgba::rgb(0, 0, 255));
        surface.set(1, 1, Rgba::rgb(255, 255, 255));

        let bytes = surface.as_bytes();
        assert_eq!(bytes.len(), 16); // 4 pixels * 4 bytes

        // First pixel: red (255, 0, 0, 255)
        assert_eq!(&bytes[0..4], &[255, 0, 0, 255]);
        // Second pixel: green (0, 255, 0, 255)
        assert_eq!(&bytes[4..8], &[0, 255, 0, 255]);
    }

    #[test]
    fn rgba_size() {
        assert_eq!(std::mem::size_of::<Rgba>(), 4);
    }
}
