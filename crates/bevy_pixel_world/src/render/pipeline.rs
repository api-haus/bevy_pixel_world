//! GPU texture creation and upload for surfaces.
//!
//! Provides utilities to create Bevy textures from surfaces and upload pixel
//! data.

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite_render::MeshMaterial2d;

use super::material::ChunkMaterial;
use crate::palette::GlobalPalette;
use crate::pixel::PixelSurface;
use crate::primitives::RgbaSurface;

/// Creates an RGBA8 texture with nearest-neighbor sampling.
///
/// Returns a handle to the created image.
pub fn create_texture(images: &mut Assets<Image>, width: u32, height: u32) -> Handle<Image> {
  let size = Extent3d {
    width,
    height,
    depth_or_array_layers: 1,
  };

  let mut image = Image::new_fill(
    size,
    TextureDimension::D2,
    &[0, 0, 0, 255],
    TextureFormat::Rgba8UnormSrgb,
    RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
  );

  // Use nearest-neighbor sampling for pixel-perfect rendering
  image.sampler = ImageSampler::nearest();

  images.add(image)
}

/// Uploads surface pixel data to an existing texture.
///
/// The surface dimensions must match the texture dimensions.
pub fn upload_surface(surface: &RgbaSurface, image: &mut Image) {
  let bytes = surface.as_bytes();
  if let Some(ref mut data) = image.data {
    data.copy_from_slice(bytes);
  }
}

/// Creates a 256x1 palette texture for GPU-side color lookup.
///
/// Each index (0-255) maps directly to a palette color.
pub fn create_palette_texture(images: &mut Assets<Image>) -> Handle<Image> {
  crate::palette::create_palette_texture(images)
}

/// Uploads palette data from GlobalPalette to a palette texture.
pub fn upload_palette(palette: &GlobalPalette, image: &mut Image) {
  crate::palette::upload_palette(palette, image);
}

/// Creates a texture for raw pixel data (Rgba8Uint format).
///
/// This format stores pixel data as unsigned integers without normalization,
/// allowing the shader to read material/color indices directly.
pub fn create_pixel_texture(images: &mut Assets<Image>, width: u32, height: u32) -> Handle<Image> {
  let size = Extent3d {
    width,
    height,
    depth_or_array_layers: 1,
  };

  let mut image = Image::new_fill(
    size,
    TextureDimension::D2,
    &[0, 0, 0, 0],
    TextureFormat::Rgba8Uint,
    RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
  );

  // No sampler needed for integer textures (use textureLoad)
  image.sampler = ImageSampler::nearest();

  images.add(image)
}

/// Uploads raw pixel data to a pixel texture.
///
/// Copies PixelSurface bytes directly (material, color, damage, flags per
/// pixel).
pub fn upload_pixels(pixels: &PixelSurface, image: &mut Image) {
  let bytes = pixels.as_bytes();
  if let Some(ref mut data) = image.data {
    data.copy_from_slice(bytes);
  }
}

/// Creates a heat texture (R8Unorm) with linear sampling for bilinear
/// interpolation.
pub fn create_heat_texture(images: &mut Assets<Image>, width: u32, height: u32) -> Handle<Image> {
  let size = Extent3d {
    width,
    height,
    depth_or_array_layers: 1,
  };

  let mut image = Image::new_fill(
    size,
    TextureDimension::D2,
    &[0],
    TextureFormat::R8Unorm,
    RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
  );

  // Linear sampling for bilinear interpolation of heat values
  image.sampler = ImageSampler::linear();

  images.add(image)
}

/// Uploads heat layer data to a heat texture.
pub fn upload_heat(heat: &[u8], image: &mut Image) {
  if let Some(ref mut data) = image.data {
    data.copy_from_slice(heat);
  }
}

/// Creates a quad mesh with Y+ up UV coordinates and origin at bottom-left.
///
/// Unlike Bevy's default Rectangle which is centered, this quad has its
/// origin at the bottom-left corner (0,0). This allows chunks to be positioned
/// at exact integer coordinates without center offset calculations that could
/// introduce float precision errors.
///
/// UV (0,0) is at bottom-left to match our Y+ up convention.
pub fn create_chunk_quad(width: f32, height: f32) -> Mesh {
  Mesh::new(
    PrimitiveTopology::TriangleList,
    RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
  )
  .with_inserted_attribute(
    Mesh::ATTRIBUTE_POSITION,
    vec![
      [0.0, 0.0, 0.0],      // bottom-left (origin)
      [width, 0.0, 0.0],    // bottom-right
      [width, height, 0.0], // top-right
      [0.0, height, 0.0],   // top-left
    ],
  )
  .with_inserted_attribute(
    Mesh::ATTRIBUTE_UV_0,
    vec![
      [0.0, 0.0], // bottom-left -> UV (0, 0)
      [1.0, 0.0], // bottom-right -> UV (1, 0)
      [1.0, 1.0], // top-right -> UV (1, 1)
      [0.0, 1.0], // top-left -> UV (0, 1)
    ],
  )
  .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]))
}

/// Spawns a chunk for static (non-updating) display.
///
/// Creates a textured quad using the custom ChunkMaterial shader with
/// GPU-side palette lookup.
pub fn spawn_static_chunk(
  commands: &mut Commands,
  images: &mut Assets<Image>,
  meshes: &mut Assets<Mesh>,
  materials: &mut Assets<ChunkMaterial>,
  palette: &GlobalPalette,
  pixels: &PixelSurface,
  display_size: Vec2,
) -> Entity {
  // Create and upload pixel texture (raw pixel data)
  let pixel_texture = create_pixel_texture(images, pixels.width(), pixels.height());
  if let Some(image) = images.get_mut(&pixel_texture) {
    upload_pixels(pixels, image);
  }

  // Create and upload palette texture
  let palette_texture = create_palette_texture(images);
  if let Some(image) = images.get_mut(&palette_texture) {
    upload_palette(palette, image);
  }

  // Create mesh with Y+ up UVs
  let mesh_handle = meshes.add(create_chunk_quad(display_size.x, display_size.y));
  // Create a 1x1 zero heat texture (no heat overlay for static chunks)
  let heat_texture = create_heat_texture(images, 1, 1);
  let material_handle = materials.add(ChunkMaterial {
    pixel_texture: Some(pixel_texture),
    palette_texture: Some(palette_texture),
    heat_texture: Some(heat_texture),
  });

  // Spawn entity
  commands
    .spawn((Mesh2d(mesh_handle), MeshMaterial2d(material_handle)))
    .id()
}

/// Convert simulation pixels to renderable RGBA.
pub fn materialize(pixels: &PixelSurface, palette: &GlobalPalette, output: &mut RgbaSurface) {
  for y in 0..pixels.height() {
    for x in 0..pixels.width() {
      let pixel = pixels[(x, y)];
      let rgba = palette.color(pixel.color.0);
      output.set(x, y, rgba);
    }
  }
}
