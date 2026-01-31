//! PNG to PixelBody conversion.
//!
//! Loads PNG images and converts them to pixel body data using Bevy's asset
//! system.

use bevy::prelude::*;

use super::PixelBody;
use crate::coords::{ColorIndex, MaterialId};
use crate::material::ids as material_ids;
use crate::palette::GlobalPalette;
use crate::pixel::Pixel;

/// Finds the best matching color within a material's 8-color palette.
///
/// Returns a ColorIndex that will map to the best palette offset when
/// the shader computes `material_id * 8 + (color_index * 7 / 255)`.
fn find_best_material_color(
  r: u8,
  g: u8,
  b: u8,
  material: MaterialId,
  palette: &GlobalPalette,
) -> ColorIndex {
  let base_idx = (material.0 as usize) * 8;
  let mut best_offset = 0u8;
  let mut best_dist = u32::MAX;

  // Check each of the material's 8 palette colors
  for offset in 0..8 {
    let palette_idx = base_idx + offset;
    if palette_idx >= 256 {
      break;
    }

    let pc = palette.colors[palette_idx];
    let dr = (r as i32 - pc.red as i32).unsigned_abs();
    let dg = (g as i32 - pc.green as i32).unsigned_abs();
    let db = (b as i32 - pc.blue as i32).unsigned_abs();
    let dist = dr * dr + dg * dg + db * db;

    if dist < best_dist {
      best_dist = dist;
      best_offset = offset as u8;
    }
  }

  // Convert palette offset (0-7) to ColorIndex that shader will map back
  // Shader does: offset = color_index * 7 / 255
  // We want: color_index such that color_index * 7 / 255 = best_offset
  // Use the middle of each range for more stable mapping
  let color_index = match best_offset {
    0 => 18,  // 0-36 maps to 0
    1 => 54,  // 37-72 maps to 1
    2 => 91,  // 73-109 maps to 2
    3 => 127, // 110-145 maps to 3
    4 => 163, // 146-181 maps to 4
    5 => 200, // 182-218 maps to 5
    6 => 236, // 219-254 maps to 6
    _ => 255, // 255 maps to 7
  };

  ColorIndex(color_index)
}

/// Loader for converting images to pixel bodies.
pub struct PixelBodyLoader;

impl PixelBodyLoader {
  /// Creates a PixelBody from a loaded Image asset using the global palette.
  ///
  /// Converts RGBA pixels to material + color:
  /// - Alpha < 128: void (not in shape mask)
  /// - Alpha >= 128: solid material with color index from palette LUT
  ///
  /// The material defaults to STONE for solid pixels.
  pub fn from_image(image: &Image, palette: &GlobalPalette) -> Option<PixelBody> {
    Self::from_image_with_material(image, material_ids::STONE, palette)
  }

  /// Creates a PixelBody from a loaded Image asset with a specific material.
  ///
  /// Converts RGBA pixels to the specified material + color:
  /// - Alpha < 128: void (not in shape mask)
  /// - Alpha >= 128: specified material with color index from palette LUT
  pub fn from_image_with_material(
    image: &Image,
    material: MaterialId,
    palette: &GlobalPalette,
  ) -> Option<PixelBody> {
    let width = image.width();
    let height = image.height();

    if width == 0 || height == 0 {
      return None;
    }

    let mut body = PixelBody::new(width, height);

    // Get raw pixel data - we expect RGBA8 format
    let Some(ref data) = image.data else {
      // No image data, treat as empty
      return Some(body);
    };

    let bytes_per_pixel = data.len() / (width as usize * height as usize);

    if bytes_per_pixel < 4 {
      // Not enough channels, treat as solid with no alpha
      for y in 0..height {
        for x in 0..width {
          let color = ColorIndex(128);
          body.set_pixel(x, y, Pixel::new(material, color));
        }
      }
      return Some(body);
    }

    // Image data is typically top-to-bottom, but our coordinate system is
    // bottom-to-top (Y+ up). Flip during conversion.
    for img_y in 0..height {
      let surface_y = height - 1 - img_y;
      for x in 0..width {
        let idx = ((img_y * width + x) as usize) * bytes_per_pixel;
        let r = data[idx];
        let g = data[idx + 1];
        let b = data[idx + 2];
        let a = data[idx + 3];

        if a < 128 {
          // Transparent - leave as void (shape_mask stays false)
          continue;
        }

        // Find the best match within the material's 8-color palette
        let color = find_best_material_color(r, g, b, material, palette);

        body.set_pixel(x, surface_y, Pixel::new(material, color));
      }
    }

    Some(body)
  }

  /// Creates a simple rectangular pixel body for testing.
  ///
  /// Fills the entire surface with the specified material.
  pub fn rectangle(width: u32, height: u32, material: MaterialId) -> PixelBody {
    let mut body = PixelBody::new(width, height);
    let pixel = Pixel::new(material, ColorIndex(128));

    for y in 0..height {
      for x in 0..width {
        body.set_pixel(x, y, pixel);
      }
    }

    body
  }

  /// Creates a circular pixel body for testing.
  ///
  /// Fills pixels within the circle radius with the specified material.
  pub fn circle(radius: u32, material: MaterialId) -> PixelBody {
    let diameter = radius * 2;
    let mut body = PixelBody::new(diameter, diameter);
    let pixel = Pixel::new(material, ColorIndex(128));
    let center = radius as f32;
    let radius_sq = (radius as f32 - 0.5).powi(2);

    for y in 0..diameter {
      for x in 0..diameter {
        let dx = x as f32 + 0.5 - center;
        let dy = y as f32 + 0.5 - center;
        if dx * dx + dy * dy <= radius_sq {
          body.set_pixel(x, y, pixel);
        }
      }
    }

    body
  }
}
