//! Signed distance field computation for terrain layers.

use crate::pixel_world::primitives::Surface;

/// Get neighbor distance +1, or u8::MAX if out of bounds.
fn neighbor_distance(dist: &Surface<u8>, x: u32, y: u32, dx: i32, dy: i32) -> u8 {
  let nx = x as i32 + dx;
  let ny = y as i32 + dy;
  if nx >= 0 && nx < dist.width() as i32 && ny >= 0 && ny < dist.height() as i32 {
    dist[(nx as u32, ny as u32)].saturating_add(1)
  } else {
    u8::MAX
  }
}

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
      let d = dist[(x, y)]
        .min(neighbor_distance(&dist, x, y, -1, 0))
        .min(neighbor_distance(&dist, x, y, 0, -1));
      dist.set(x, y, d);
    }
  }

  // Backward pass
  for y in (0..h).rev() {
    for x in (0..w).rev() {
      let d = dist[(x, y)]
        .min(neighbor_distance(&dist, x, y, 1, 0))
        .min(neighbor_distance(&dist, x, y, 0, 1));
      dist.set(x, y, d);
    }
  }

  dist
}
