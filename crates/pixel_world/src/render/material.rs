//! Custom Material2d for chunk rendering.

use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::sprite_render::Material2d;

/// Material for rendering chunks with a texture.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct ChunkMaterial {
  #[texture(0)]
  #[sampler(1)]
  pub texture: Option<Handle<Image>>,
}

impl Material2d for ChunkMaterial {
  fn fragment_shader() -> ShaderRef {
    "embedded://pixel_world/render/shaders/chunk.wgsl".into()
  }
}
