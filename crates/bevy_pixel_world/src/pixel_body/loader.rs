//! PNG to PixelBody conversion.
//!
//! Loads PNG images and converts them to pixel body data using Bevy's asset
//! system.

use bevy::prelude::*;

use super::PixelBody;
use crate::coords::{ColorIndex, MaterialId};
use crate::material::ids as material_ids;
use crate::pixel::Pixel;

/// Loader for converting images to pixel bodies.
pub struct PixelBodyLoader;

impl PixelBodyLoader {
  /// Creates a PixelBody from a loaded Image asset.
  ///
  /// Converts RGBA pixels to material + color:
  /// - Alpha < 128: void (not in shape mask)
  /// - Alpha >= 128: solid material with color derived from RGB luminance
  ///
  /// The material defaults to STONE for solid pixels.
  pub fn from_image(image: &Image) -> Option<PixelBody> {
    Self::from_image_with_material(image, material_ids::STONE)
  }

  /// Creates a PixelBody from a loaded Image asset with a specific material.
  ///
  /// Converts RGBA pixels to the specified material + color:
  /// - Alpha < 128: void (not in shape mask)
  /// - Alpha >= 128: specified material with color derived from RGB luminance
  pub fn from_image_with_material(image: &Image, material: MaterialId) -> Option<PixelBody> {
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

        // Convert RGB to a luminance-based color index (0-255)
        // Using standard luminance coefficients: 0.299*R + 0.587*G + 0.114*B
        let luminance = (r as f32 * 0.299 + g as f32 * 0.587 + b as f32 * 0.114) as u8;
        let color = ColorIndex(luminance);

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
