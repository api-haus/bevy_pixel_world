//! Configuration for pixel-perfect camera rendering.

use bevy::prelude::*;

/// How pixel size is determined for the low-resolution render target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PixelSizeMode {
  /// Fixed vertical resolution in pixels.
  /// Width calculated from aspect ratio.
  FixedVerticalResolution(u32),

  /// World units per pixel (typically 1.0).
  /// Resolution derived from camera orthographic size.
  WorldSpacePixelSize(f32),
}

impl Default for PixelSizeMode {
  fn default() -> Self {
    // Default to 1 world unit = 1 pixel
    Self::WorldSpacePixelSize(1.0)
  }
}

/// Configuration for the pixel camera plugin.
#[derive(Resource, Clone, Debug)]
pub struct PixelCameraConfig {
  /// How pixel size is determined.
  pub pixel_size_mode: PixelSizeMode,

  /// Margin pixels around render target for subpixel offset.
  /// Typically 1-2 pixels.
  pub margin: u32,

  /// Enable subpixel smoothing.
  /// When false, camera snaps without offset compensation.
  pub subpixel_smoothing: bool,

  /// Render egui at full resolution (not pixelated).
  /// When true, egui renders to the blit camera instead of the scene camera.
  /// Requires bevy_egui plugin to be added to the app.
  pub egui_full_resolution: bool,
}

impl Default for PixelCameraConfig {
  fn default() -> Self {
    Self {
      pixel_size_mode: PixelSizeMode::default(),
      margin: 2,
      subpixel_smoothing: true,
      egui_full_resolution: true,
    }
  }
}
