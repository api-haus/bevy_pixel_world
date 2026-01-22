//! Fragment-shader-style drawing API for surfaces.
//!
//! [`Blitter`] provides methods to draw into a [`Surface`] using closures
//! that receive both absolute coordinates (x, y) and normalized UV coordinates.

use crate::surface::Surface;

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
        Self { x, y, width, height }
    }

    /// Creates a rectangle covering an entire surface.
    #[inline]
    pub fn full(surface_width: u32, surface_height: u32) -> Self {
        Self::new(0, 0, surface_width, surface_height)
    }

    /// Clamps this rect to fit within the given bounds.
    fn clamped(&self, bound_width: u32, bound_height: u32) -> Self {
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

/// Drawing API for surfaces.
pub struct Blitter<'a, T> {
    surface: &'a mut Surface<T>,
}

impl<'a, T> Blitter<'a, T> {
    /// Creates a new blitter for the given surface.
    pub fn new(surface: &'a mut Surface<T>) -> Self {
        Self { surface }
    }

    /// Fills a rectangle with a closure that receives coordinates.
    ///
    /// For each pixel in `rect`, calls `f(x, y, u, v)` where:
    /// - `x, y` are absolute coordinates within the surface
    /// - `u, v` are normalized coordinates within the rect (0.0 to 1.0)
    ///
    /// The rect is clamped to surface bounds; out-of-bounds portions are skipped.
    pub fn blit<F>(&mut self, rect: Rect, mut f: F)
    where
        F: FnMut(u32, u32, f32, f32) -> T,
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
                let value = f(x, y, u, v);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::Rgba;

    #[test]
    fn blit_provides_correct_uv() {
        let mut surface = Surface::<(u32, u32, f32, f32)>::filled(10, 10, (0, 0, 0.0, 0.0));
        let mut blitter = Blitter::new(&mut surface);

        let rect = Rect::new(2, 3, 4, 3);
        blitter.blit(rect, |x, y, u, v| (x, y, u, v));

        // Check corners
        let tl = surface.get(2, 3).unwrap();
        assert_eq!(tl.0, 2); // x
        assert_eq!(tl.1, 3); // y
        assert!((tl.2 - 0.0).abs() < 0.001); // u = 0
        assert!((tl.3 - 0.0).abs() < 0.001); // v = 0

        let tr = surface.get(5, 3).unwrap(); // x = 2 + 3 = 5 (last column)
        assert_eq!(tr.0, 5);
        assert_eq!(tr.1, 3);
        assert!((tr.2 - 1.0).abs() < 0.001); // u = 1

        let bl = surface.get(2, 5).unwrap(); // y = 3 + 2 = 5 (last row)
        assert_eq!(bl.0, 2);
        assert_eq!(bl.1, 5);
        assert!((bl.3 - 1.0).abs() < 0.001); // v = 1

        let br = surface.get(5, 5).unwrap();
        assert!((br.2 - 1.0).abs() < 0.001); // u = 1
        assert!((br.3 - 1.0).abs() < 0.001); // v = 1
    }

    #[test]
    fn blit_clamps_out_of_bounds() {
        let mut surface = Surface::<u8>::filled(10, 10, 0);
        let mut blitter = Blitter::new(&mut surface);

        // Rect extends past the surface
        let rect = Rect::new(8, 8, 5, 5); // Would go to (12, 12)
        blitter.blit(rect, |_, _, _, _| 42);

        // Only (8,8), (8,9), (9,8), (9,9) should be set
        assert_eq!(surface.get(8, 8), Some(&42));
        assert_eq!(surface.get(9, 9), Some(&42));
        // Outside original rect area should be 0
        assert_eq!(surface.get(7, 7), Some(&0));
        // Check we didn't panic
    }

    #[test]
    fn blit_rect_completely_outside() {
        let mut surface = Surface::<u8>::filled(10, 10, 0);
        let mut blitter = Blitter::new(&mut surface);

        // Completely outside
        let rect = Rect::new(20, 20, 5, 5);
        blitter.blit(rect, |_, _, _, _| 42);

        // Nothing should change
        assert_eq!(surface.get(0, 0), Some(&0));
    }

    #[test]
    fn fill_and_clear() {
        let mut surface = Surface::<Rgba>::new(5, 5);

        Blitter::new(&mut surface).fill(Rect::new(1, 1, 2, 2), Rgba::rgb(255, 0, 0));
        assert_eq!(surface.get(1, 1), Some(&Rgba::rgb(255, 0, 0)));
        assert_eq!(surface.get(0, 0), Some(&Rgba::default()));

        Blitter::new(&mut surface).clear(Rgba::rgb(0, 255, 0));
        assert_eq!(surface.get(0, 0), Some(&Rgba::rgb(0, 255, 0)));
        assert_eq!(surface.get(4, 4), Some(&Rgba::rgb(0, 255, 0)));
    }
}
