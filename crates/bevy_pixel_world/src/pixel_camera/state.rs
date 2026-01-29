//! Runtime state for pixel camera rendering.

use bevy::prelude::*;

/// Runtime state for pixel camera rendering.
#[derive(Resource, Default)]
pub struct PixelCameraState {
  /// Low-resolution render target.
  pub render_target: Handle<Image>,

  /// Current subpixel offset in UV space.
  pub subpixel_offset_uv: Vec2,

  /// Calculated world-space pixel size.
  pub pixel_world_size: f32,

  /// Low-res target dimensions (including margin).
  pub target_size: UVec2,

  /// Whether the pixel camera has been initialized.
  pub initialized: bool,
}
