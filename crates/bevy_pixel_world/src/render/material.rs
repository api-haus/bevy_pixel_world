//! Custom Material2d for chunk rendering.

use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d};

/// Material for rendering chunks with GPU-side palette lookup.
///
/// Uses raw pixel data (material/color indices) and a palette texture
/// to resolve colors in the fragment shader.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct ChunkMaterial {
  /// Raw pixel data (Rgba8Uint): [material, color, damage, flags]
  #[texture(0, sample_type = "u_int")]
  pub pixel_texture: Option<Handle<Image>>,

  /// Palette lookup texture (256x1 Rgba8UnormSrgb)
  #[texture(1)]
  #[sampler(2)]
  pub palette_texture: Option<Handle<Image>>,

  /// Heat layer texture (128x128 R8Unorm, bilinear sampled)
  #[texture(3)]
  #[sampler(4)]
  pub heat_texture: Option<Handle<Image>>,
}

impl Material2d for ChunkMaterial {
  fn fragment_shader() -> ShaderRef {
    "embedded://bevy_pixel_world/render/shaders/chunk.wgsl".into()
  }

  fn alpha_mode(&self) -> AlphaMode2d {
    AlphaMode2d::Blend
  }
}
