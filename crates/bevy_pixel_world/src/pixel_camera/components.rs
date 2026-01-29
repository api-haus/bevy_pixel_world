//! Components for pixel-perfect camera rendering.

use bevy::prelude::*;

use super::config::PixelSizeMode;

/// Marker for camera that uses pixel-perfect rendering.
#[derive(Component, Default)]
pub struct PixelCamera {
  /// Override pixel size mode for this camera.
  pub pixel_size_mode: Option<PixelSizeMode>,
}

/// Internal: tracks logical camera position before snapping.
///
/// Streaming systems use this to get the smooth camera position
/// for chunk loading, avoiding pop-in at pixel grid boundaries.
#[derive(Component, Default, Clone, Copy)]
pub struct LogicalCameraPosition(pub Vec2);
