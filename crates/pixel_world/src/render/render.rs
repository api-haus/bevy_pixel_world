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
use crate::RgbaSurface;
use crate::material::Materials;
use crate::pixel::PixelSurface;
use crate::render::Rgba;

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
/// Each material occupies 8 consecutive RGBA entries (material 0 uses indices
/// 0-7, material 1 uses 8-15, etc). Supports up to 32 materials.
pub fn create_palette_texture(images: &mut Assets<Image>) -> Handle<Image> {
  let size = Extent3d {
    width: 256,
    height: 1,
    depth_or_array_layers: 1,
  };

  let mut image = Image::new_fill(
    size,
    TextureDimension::D2,
    &[0, 0, 0, 255],
    TextureFormat::Rgba8UnormSrgb,
    RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
  );

  // Use nearest-neighbor sampling for exact color lookup
  image.sampler = ImageSampler::nearest();

  images.add(image)
}

/// Uploads palette data from Materials to a palette texture.
pub fn upload_palette(materials: &Materials, image: &mut Image) {
  let Some(ref mut data) = image.data else {
    return;
  };

  // Clear to black
  data.fill(0);

  // Populate palette: each material gets 8 consecutive RGBA entries
  // Supports up to 32 materials (256 / 8 = 32)
  let count = materials.len().min(32);
  for material_id in 0..count {
    let material = materials.get(crate::coords::MaterialId(material_id as u8));
    let base = material_id * 8 * 4; // 8 colors * 4 bytes per color

    for (color_idx, &Rgba { r, g, b, a }) in material.palette.iter().enumerate() {
      let offset = base + color_idx * 4;
      if offset + 4 <= data.len() {
        data[offset] = r;
        data[offset + 1] = g;
        data[offset + 2] = b;
        data[offset + 3] = a;
      }
    }
  }
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

/// Creates a quad mesh with Y+ up UV coordinates.
///
/// Unlike Bevy's default Rectangle which has UV (0,0) at top-left,
/// this quad has UV (0,0) at bottom-left to match our Y+ up convention.
/// This allows the shader to sample directly without Y-flipping.
pub fn create_chunk_quad(width: f32, height: f32) -> Mesh {
  let hw = width / 2.0;
  let hh = height / 2.0;

  Mesh::new(
    PrimitiveTopology::TriangleList,
    RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
  )
  .with_inserted_attribute(
    Mesh::ATTRIBUTE_POSITION,
    vec![
      [-hw, -hh, 0.0], // bottom-left
      [hw, -hh, 0.0],  // bottom-right
      [hw, hh, 0.0],   // top-right
      [-hw, hh, 0.0],  // top-left
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
  material_registry: &Materials,
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
    upload_palette(material_registry, image);
  }

  // Create mesh with Y+ up UVs
  let mesh_handle = meshes.add(create_chunk_quad(display_size.x, display_size.y));
  let material_handle = materials.add(ChunkMaterial {
    pixel_texture: Some(pixel_texture),
    palette_texture: Some(palette_texture),
  });

  // Spawn entity
  commands
    .spawn((Mesh2d(mesh_handle), MeshMaterial2d(material_handle)))
    .id()
}

/// Convert simulation pixels to renderable RGBA.
pub fn materialize(pixels: &PixelSurface, materials: &Materials, output: &mut RgbaSurface) {
  for y in 0..pixels.height() {
    for x in 0..pixels.width() {
      let pixel = pixels[(x, y)];
      let material = materials.get(pixel.material);
      let rgba = material.sample(pixel.color);
      output.set(x, y, rgba);
    }
  }
}
