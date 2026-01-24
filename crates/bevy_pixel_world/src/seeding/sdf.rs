//! Signed distance field computation for terrain layers.

use crate::primitives::Surface;

/// Compute distance to nearest void (value 0) pixel.
/// Returns u8 distance clamped to 255.
pub fn distance_to_void(mask: &Surface<u8>) -> Surface<u8> {
  // Simple 2-pass distance transform (Rosenfeld-Pfaltz)
  // Pass 1: top-left to bottom-right
  // Pass 2: bottom-right to top-left
  // Good enough for ~16 pixel soil depth

  let w = mask.width();
  let h = mask.height();
  let mut dist = Surface::<u8>::new(w, h);

  // Initialize: 0 for void, 255 for solid
  for y in 0..h {
    for x in 0..w {
      dist.set(x, y, if mask[(x, y)] == 0 { 0 } else { 255 });
    }
  }

  // Forward pass
  for y in 0..h {
    for x in 0..w {
      let mut d = dist[(x, y)];
      if x > 0 {
        d = d.min(dist[(x - 1, y)].saturating_add(1));
      }
      if y > 0 {
        d = d.min(dist[(x, y - 1)].saturating_add(1));
      }
      dist.set(x, y, d);
    }
  }

  // Backward pass
  for y in (0..h).rev() {
    for x in (0..w).rev() {
      let mut d = dist[(x, y)];
      if x < w - 1 {
        d = d.min(dist[(x + 1, y)].saturating_add(1));
      }
      if y < h - 1 {
        d = d.min(dist[(x, y + 1)].saturating_add(1));
      }
      dist.set(x, y, d);
    }
  }

  dist
}
