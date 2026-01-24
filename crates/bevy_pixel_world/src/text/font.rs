//! Font rendering for surfaces.
//!
//! Uses `ab_glyph` for font rasterization. Provides functions to render text
//! as a coverage mask and stamp it onto an [`RgbaSurface`].

use ab_glyph::{Font, FontRef, Glyph, PxScale, ScaleFont};

use crate::primitives::RgbaSurface;
use crate::render::Rgba;

/// Default font embedded in the binary.
const DEFAULT_FONT_BYTES: &[u8] = include_bytes!("fonts/ProggyClean.ttf");

/// A CPU-side font for text rasterization.
pub struct CpuFont {
  font: FontRef<'static>,
}

impl CpuFont {
  /// Creates a font from the embedded default (ProggyClean).
  pub fn default_font() -> Self {
    Self {
      font: FontRef::try_from_slice(DEFAULT_FONT_BYTES).expect("embedded font should be valid"),
    }
  }

  /// Creates a font from raw TTF/OTF bytes.
  ///
  /// Returns `None` if the bytes are not a valid font.
  pub fn from_bytes(data: &'static [u8]) -> Option<Self> {
    FontRef::try_from_slice(data).ok().map(|font| Self { font })
  }
}

/// A boolean coverage mask from rasterized text.
///
/// Pixels with coverage above a threshold are marked as `true`.
pub struct TextMask {
  data: Vec<bool>,
  width: u32,
  height: u32,
}

impl TextMask {
  /// Returns the width of the mask in pixels.
  pub fn width(&self) -> u32 {
    self.width
  }

  /// Returns the height of the mask in pixels.
  pub fn height(&self) -> u32 {
    self.height
  }

  /// Returns whether the pixel at (x, y) is covered.
  ///
  /// Returns `false` for out-of-bounds coordinates.
  pub fn get(&self, x: u32, y: u32) -> bool {
    if x < self.width && y < self.height {
      self.data[(y as usize) * (self.width as usize) + (x as usize)]
    } else {
      false
    }
  }
}

/// Positions glyphs along the baseline for the given text.
fn layout_glyphs<SF: ScaleFont<F>, F: Font>(
  scaled_font: &SF,
  text: &str,
  scale: PxScale,
  char_spacing: f32,
) -> Vec<Glyph> {
  let mut glyphs = Vec::new();
  let mut cursor_x = 0.0f32;

  for ch in text.chars() {
    let glyph_id = scaled_font.glyph_id(ch);
    let glyph =
      glyph_id.with_scale_and_position(scale, ab_glyph::point(cursor_x, scaled_font.ascent()));
    cursor_x += scaled_font.h_advance(glyph_id) + char_spacing;
    glyphs.push(glyph);
  }

  glyphs
}

/// Computes the aggregate bounding box for a list of glyphs.
///
/// Returns `Some((min_x, min_y, max_x, max_y))` or `None` if bounds collapse.
fn compute_glyph_bounds<SF: ScaleFont<F>, F: Font>(
  scaled_font: &SF,
  glyphs: &[Glyph],
) -> Option<(i32, i32, i32, i32)> {
  let mut min_x = i32::MAX;
  let mut min_y = i32::MAX;
  let mut max_x = i32::MIN;
  let mut max_y = i32::MIN;

  for glyph in glyphs {
    if let Some(outlined) = scaled_font.outline_glyph(glyph.clone()) {
      let bounds = outlined.px_bounds();
      min_x = min_x.min(bounds.min.x.floor() as i32);
      min_y = min_y.min(bounds.min.y.floor() as i32);
      max_x = max_x.max(bounds.max.x.ceil() as i32);
      max_y = max_y.max(bounds.max.y.ceil() as i32);
    }
  }

  if min_x >= max_x || min_y >= max_y {
    None
  } else {
    Some((min_x, min_y, max_x, max_y))
  }
}

/// Rasterizes glyphs into a boolean coverage mask.
fn rasterize_glyphs<SF: ScaleFont<F>, F: Font>(
  scaled_font: &SF,
  glyphs: Vec<Glyph>,
  min_x: i32,
  min_y: i32,
  width: u32,
  height: u32,
) -> Vec<bool> {
  let mut data = vec![false; (width * height) as usize];

  for glyph in glyphs {
    if let Some(outlined) = scaled_font.outline_glyph(glyph) {
      let bounds = outlined.px_bounds();
      outlined.draw(|px, py, coverage| {
        if coverage > 0.5 {
          let x = (bounds.min.x.floor() as i32 + px as i32 - min_x) as u32;
          let y = (bounds.min.y.floor() as i32 + py as i32 - min_y) as u32;
          if x < width && y < height {
            data[(y as usize) * (width as usize) + (x as usize)] = true;
          }
        }
      });
    }
  }

  data
}

/// Rasterizes text into a coverage mask.
///
/// - `font_scale`: Font size in pixels (e.g., 16.0 for 16px).
/// - `char_spacing`: Extra spacing between characters in pixels.
///
/// Returns `None` if the text is empty or contains no renderable glyphs.
pub fn rasterize_text(
  font: &CpuFont,
  text: &str,
  font_scale: f32,
  char_spacing: f32,
) -> Option<TextMask> {
  if text.is_empty() {
    return None;
  }

  let scale = PxScale::from(font_scale);
  let scaled_font = font.font.as_scaled(scale);

  let glyphs = layout_glyphs(&scaled_font, text, scale, char_spacing);
  if glyphs.is_empty() {
    return None;
  }

  let (min_x, min_y, max_x, max_y) = compute_glyph_bounds(&scaled_font, &glyphs)?;

  let width = (max_x - min_x) as u32;
  let height = (max_y - min_y) as u32;
  let data = rasterize_glyphs(&scaled_font, glyphs, min_x, min_y, width, height);

  Some(TextMask {
    data,
    width,
    height,
  })
}

/// Text rendering style configuration.
///
/// Bundles font scale, character spacing, and color for text rendering.
pub struct TextStyle {
  /// Font size in pixels (e.g., 16.0 for 16px).
  pub font_scale: f32,
  /// Extra spacing between characters in pixels.
  pub char_spacing: f32,
  /// Color to use for the text.
  pub color: Rgba,
}

impl Default for TextStyle {
  fn default() -> Self {
    Self {
      font_scale: 16.0,
      char_spacing: 0.0,
      color: Rgba::new(255, 255, 255, 255),
    }
  }
}

/// Stamps a text mask onto a surface at the given position.
///
/// - `x, y`: Position of the bottom-left corner of the text.
/// - `color`: Color to use for covered pixels.
///
/// The mask is flipped vertically to match the surface's Y+ up coordinate
/// system.
pub fn stamp_text(surface: &mut RgbaSurface, mask: &TextMask, x: i32, y: i32, color: Rgba) {
  let surf_width = surface.width() as i32;
  let surf_height = surface.height() as i32;
  let mask_height = mask.height() as i32;

  for my in 0..mask.height() {
    for mx in 0..mask.width() {
      if mask.get(mx, my) {
        // Flip Y: mask row 0 (top in ab_glyph) goes to y + mask_height - 1 (top in
        // surface)
        let surf_x = x + mx as i32;
        let surf_y = y + (mask_height - 1 - my as i32);

        if surf_x >= 0 && surf_x < surf_width && surf_y >= 0 && surf_y < surf_height {
          surface.set(surf_x as u32, surf_y as u32, color);
        }
      }
    }
  }
}

/// Renders text directly onto a surface.
///
/// Convenience function combining [`rasterize_text`] and [`stamp_text`].
///
/// - `x, y`: Position of the bottom-left corner of the text.
/// - `style`: Text style (font scale, spacing, color).
pub fn draw_text(
  surface: &mut RgbaSurface,
  font: &CpuFont,
  text: &str,
  x: i32,
  y: i32,
  style: &TextStyle,
) {
  if let Some(mask) = rasterize_text(font, text, style.font_scale, style.char_spacing) {
    stamp_text(surface, &mask, x, y, style.color);
  }
}
