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
use crate::material::Materials;
use crate::pixel::PixelSurface;
use crate::RgbaSurface;

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
/// Creates a textured quad using the custom ChunkMaterial shader.
/// For dynamic chunks that update every frame, manually manage the texture
/// and material as shown in the `uv_quad` example.
pub fn spawn_static_chunk(
  commands: &mut Commands,
  images: &mut Assets<Image>,
  meshes: &mut Assets<Mesh>,
  materials: &mut Assets<ChunkMaterial>,
  surface: &RgbaSurface,
  display_size: Vec2,
) -> Entity {
  // Create and upload texture
  let texture_handle = create_texture(images, surface.width(), surface.height());
  if let Some(image) = images.get_mut(&texture_handle) {
    upload_surface(surface, image);
  }

  // Create mesh with Y+ up UVs
  let mesh_handle = meshes.add(create_chunk_quad(display_size.x, display_size.y));
  let material_handle = materials.add(ChunkMaterial {
    texture: Some(texture_handle),
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
