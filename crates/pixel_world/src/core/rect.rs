/// A rectangular region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
  pub x: u32,
  pub y: u32,
  pub width: u32,
  pub height: u32,
}

impl Rect {
  /// Creates a new rectangle.
  #[inline]
  pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
    Self {
      x,
      y,
      width,
      height,
    }
  }

  /// Creates a rectangle covering an entire surface.
  #[inline]
  pub fn full(surface_width: u32, surface_height: u32) -> Self {
    Self::new(0, 0, surface_width, surface_height)
  }

  /// Clamps this rect to fit within the given bounds.
  pub(crate) fn clamped(&self, bound_width: u32, bound_height: u32) -> Self {
    let x = self.x.min(bound_width);
    let y = self.y.min(bound_height);
    let max_w = bound_width.saturating_sub(x);
    let max_h = bound_height.saturating_sub(y);
    Self {
      x,
      y,
      width: self.width.min(max_w),
      height: self.height.min(max_h),
    }
  }
}
