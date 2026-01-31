//! CRT shader materials for Bevy 0.17.
//!
//! Each material corresponds to a pass in the CRT rendering pipeline.

use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use bevy::sprite_render::Material2d;

/// Uniform data for texture size, shared by most passes.
#[derive(Clone, Copy, Default, ShaderType)]
pub struct TextureSizeUniform {
  pub size: Vec2,
}

/// Configurable CRT parameters passed to deconvergence shader.
///
/// Field order matters for WGSL alignment - Vec2 fields first, then scalars.
#[derive(Clone, Copy, ShaderType)]
pub struct CrtParams {
  /// Curvature amount (x, y).
  pub curvature: Vec2,
  /// Scanline intensity and sharpness.
  pub scanline: Vec2,
  /// Mask strength and type (as f32 for uniform compatibility).
  pub mask: Vec2,
  /// Glow intensity and brightness boost.
  pub glow_brightness: Vec2,
  /// Output gamma and corner size.
  pub gamma_corner: Vec2,
  /// Whether CRT effect is enabled (1 = on, 0 = bypass).
  pub enabled: u32,
}

impl Default for CrtParams {
  fn default() -> Self {
    Self {
      curvature: Vec2::new(0.03, 0.04),
      scanline: Vec2::new(0.6, 0.75),
      mask: Vec2::new(0.3, 0.0),
      glow_brightness: Vec2::new(0.08, 1.4),
      gamma_corner: Vec2::new(1.75, 0.01),
      enabled: 1,
    }
  }
}

/// Afterglow pass material - phosphor persistence effect.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct AfterglowMaterial {
  /// Source frame (delayed by N frames).
  #[texture(0)]
  #[sampler(1)]
  pub source_image: Handle<Image>,

  /// Texture dimensions.
  #[uniform(2)]
  pub texture_size: Vec2,

  /// Previous afterglow output (feedback loop).
  #[texture(3)]
  #[sampler(4)]
  pub feedback: Handle<Image>,
}

impl Material2d for AfterglowMaterial {
  fn fragment_shader() -> ShaderRef {
    "embedded://bevy_crt/shaders/afterglow.wgsl".into()
  }
}

/// Pre-shader material - color adjustments and afterglow blending.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct PreMaterial {
  /// Source image (game render).
  #[texture(0)]
  #[sampler(1)]
  pub source_image: Handle<Image>,

  /// Texture dimensions.
  #[uniform(2)]
  pub texture_size: Vec2,

  /// Afterglow pass output.
  #[texture(3)]
  #[sampler(4)]
  pub afterglow: Handle<Image>,
}

impl Material2d for PreMaterial {
  fn fragment_shader() -> ShaderRef {
    "embedded://bevy_crt/shaders/preshader.wgsl".into()
  }
}

/// Linearize material - gamma correction and interlacing.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct LinearizeMaterial {
  /// Pre-shader output.
  #[texture(0)]
  #[sampler(1)]
  pub source_image: Handle<Image>,

  /// Texture dimensions.
  #[uniform(2)]
  pub texture_size: Vec2,

  /// Current frame count (for interlacing).
  #[uniform(3)]
  pub frame_count: u32,
}

impl Material2d for LinearizeMaterial {
  fn fragment_shader() -> ShaderRef {
    "embedded://bevy_crt/shaders/linearize.wgsl".into()
  }
}

/// Pass1 material - horizontal filtering/sharpening.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct PostMaterial {
  /// Linearize pass output.
  #[texture(0)]
  #[sampler(1)]
  pub linearize_pass: Handle<Image>,

  /// Texture dimensions.
  #[uniform(2)]
  pub texture_size: Vec2,
}

impl Material2d for PostMaterial {
  fn fragment_shader() -> ShaderRef {
    "embedded://bevy_crt/shaders/pass1.wgsl".into()
  }
}

/// Horizontal bloom material.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct BloomHorizontal {
  /// Linearize pass output.
  #[texture(0)]
  #[sampler(1)]
  pub linearize_pass: Handle<Image>,

  /// Texture dimensions.
  #[uniform(2)]
  pub texture_size: Vec2,
}

impl Material2d for BloomHorizontal {
  fn fragment_shader() -> ShaderRef {
    "embedded://bevy_crt/shaders/bloom_horizontal.wgsl".into()
  }
}

/// Vertical bloom material.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct BloomVertical {
  /// Horizontal bloom output.
  #[texture(0)]
  #[sampler(1)]
  pub source_image: Handle<Image>,

  /// Texture dimensions.
  #[uniform(2)]
  pub texture_size: Vec2,
}

impl Material2d for BloomVertical {
  fn fragment_shader() -> ShaderRef {
    "embedded://bevy_crt/shaders/bloom_vertical.wgsl".into()
  }
}

/// Pass2 material - vertical filtering and scanlines.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct PostMaterial2 {
  /// Pass1 output.
  #[texture(0)]
  #[sampler(1)]
  pub pass_1: Handle<Image>,

  /// Texture dimensions.
  #[uniform(2)]
  pub texture_size: Vec2,

  /// Linearize pass output (for gamma info).
  #[texture(3)]
  #[sampler(4)]
  pub linearize_pass: Handle<Image>,

  /// Source game resolution (for pixel-aligned scanlines).
  #[uniform(5)]
  pub source_size: Vec2,
}

impl Material2d for PostMaterial2 {
  fn fragment_shader() -> ShaderRef {
    "embedded://bevy_crt/shaders/pass2.wgsl".into()
  }
}

/// Deconvergence material - final CRT pass with mask, curvature, etc.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct DeconvergenceMaterial {
  /// Pass2 output.
  #[texture(0)]
  #[sampler(1)]
  pub source_image: Handle<Image>,

  /// Texture dimensions.
  #[uniform(2)]
  pub texture_size: Vec2,

  /// Linearize pass output (for gamma info).
  #[texture(3)]
  #[sampler(4)]
  pub linearize_pass: Handle<Image>,

  /// Bloom pass output.
  #[texture(5)]
  #[sampler(6)]
  pub bloom_pass: Handle<Image>,

  /// Pre-shader pass output.
  #[texture(7)]
  #[sampler(8)]
  pub pre_pass: Handle<Image>,

  /// Current frame count (for effects).
  #[uniform(9)]
  pub frame_count: u32,

  /// Source game resolution (for pixel-aligned mask/scanlines).
  #[uniform(10)]
  pub source_size: Vec2,

  /// Configurable CRT parameters.
  #[uniform(11)]
  pub params: CrtParams,
}

impl Material2d for DeconvergenceMaterial {
  fn fragment_shader() -> ShaderRef {
    "embedded://bevy_crt/shaders/deconvergence.wgsl".into()
  }
}
