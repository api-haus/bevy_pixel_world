//! LocalFragment-shader-style drawing API for surfaces.
//!
//! [`Blitter`] provides methods to draw into a [`Surface`] using closures
//! that receive both absolute coordinates (x, y) and normalized UV coordinates.
//!
//! # Coordinate System
//!
//! Uses Y+ up coordinates consistent with [`Surface`]:
//! - `u` increases from 0.0 (left) to 1.0 (right)
//! - `v` increases from 0.0 (bottom) to 1.0 (top)

use super::surface::Surface;
use crate::primitives::rect::Rect;

/// LocalFragment data passed to blit callbacks.
///
/// See `docs/architecture/glossary.md` for coordinate system conventions.
#[derive(Clone, Copy, Debug)]
pub struct LocalFragment {
  /// Absolute X coordinate on the surface.
  pub x: u32,
  /// Absolute Y coordinate on the surface (Y+ up).
  pub y: u32,
  /// Normalized U coordinate (0.0 at left, 1.0 at right).
  pub u: f32,
  /// Normalized V coordinate (0.0 at bottom, 1.0 at top).
  pub v: f32,
}

/// Drawing API for surfaces.
pub struct Blitter<'a, T> {
  surface: &'a mut Surface<T>,
}

impl<'a, T> Blitter<'a, T> {
  /// Creates a new blitter for the given surface.
  pub fn new(surface: &'a mut Surface<T>) -> Self {
    Self { surface }
  }

  /// Fills a rectangle with a closure that receives a [`LocalFragment`].
  ///
  /// For each pixel in `rect`, calls `f(fragment)` where fragment contains:
  /// - `x, y`: absolute surface coordinates (Y+ up)
  /// - `u`: normalized X within the rect (0.0 at left, 1.0 at right)
  /// - `v`: normalized Y within the rect (0.0 at bottom, 1.0 at top)
  ///
  /// The rect is clamped to surface bounds; out-of-bounds portions are skipped.
  pub fn blit<F>(&mut self, rect: Rect, mut f: F)
  where
    F: FnMut(LocalFragment) -> T,
  {
    let rect = rect.clamped(self.surface.width(), self.surface.height());
    if rect.width == 0 || rect.height == 0 {
      return;
    }

    let w_recip = if rect.width > 1 {
      1.0 / (rect.width - 1) as f32
    } else {
      0.0
    };
    let h_recip = if rect.height > 1 {
      1.0 / (rect.height - 1) as f32
    } else {
      0.0
    };

    for dy in 0..rect.height {
      let y = rect.y + dy;
      let v = dy as f32 * h_recip;
      for dx in 0..rect.width {
        let x = rect.x + dx;
        let u = dx as f32 * w_recip;
        let frag = LocalFragment { x, y, u, v };
        let value = f(frag);
        // We know (x, y) is in bounds due to clamping
        self.surface.set(x, y, value);
      }
    }
  }

  /// Fills a rectangle with a solid value.
  pub fn fill(&mut self, rect: Rect, value: T)
  where
    T: Clone,
  {
    let rect = rect.clamped(self.surface.width(), self.surface.height());
    for y in rect.y..(rect.y + rect.height) {
      for x in rect.x..(rect.x + rect.width) {
        self.surface.set(x, y, value.clone());
      }
    }
  }

  /// Clears the entire surface with a value.
  pub fn clear(&mut self, value: T)
  where
    T: Clone,
  {
    let rect = Rect::full(self.surface.width(), self.surface.height());
    self.fill(rect, value);
  }
}
