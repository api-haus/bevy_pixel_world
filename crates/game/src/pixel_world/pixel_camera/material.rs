//! Blit material for pixel-perfect rendering.

use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use bevy::sprite_render::Material2d;

/// Uniform data for the blit shader.
#[derive(Clone, Copy, Default, ShaderType)]
pub struct PixelBlitUniforms {
  /// Subpixel offset in UV space (0.0 to 1.0 range, typically very small).
  pub subpixel_offset: Vec2,
  /// Viewport rectangle in UV space: xy = offset from margin, zw = size.
  pub viewport_rect: Vec4,
}

/// Material for blitting the low-res render target to screen.
///
/// Samples the low-res texture with a subpixel offset to create
/// smooth camera movement while maintaining pixel-perfect rendering.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct PixelBlitMaterial {
  /// Low-resolution render target texture.
  /// Note: Uses default filtering sampler but image has nearest-neighbor
  /// setting.
  #[texture(0)]
  #[sampler(1)]
  pub texture: Handle<Image>,

  /// Blit uniforms (subpixel offset and viewport rect).
  #[uniform(2)]
  pub uniforms: PixelBlitUniforms,
}

impl Material2d for PixelBlitMaterial {
  fn fragment_shader() -> ShaderRef {
    "embedded://sim2d/pixel_world/pixel_camera/shaders/blit.wgsl".into()
  }
}
