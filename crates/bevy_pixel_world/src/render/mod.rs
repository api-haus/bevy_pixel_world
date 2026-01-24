mod material;
mod render;

pub use material::ChunkMaterial;
pub use render::{
  create_chunk_quad, create_palette_texture, create_pixel_texture, create_texture, materialize,
  spawn_static_chunk, upload_palette, upload_pixels, upload_surface,
};

/// RGBA pixel with 8 bits per channel, using sRGB color space.
///
/// Re-exported from the `palette` crate for color handling.
pub type Rgba = palette::Srgba<u8>;

/// Creates an opaque RGB color (alpha = 255).
#[inline]
pub const fn rgb(r: u8, g: u8, b: u8) -> Rgba {
  Rgba::new(r, g, b, 255)
}
