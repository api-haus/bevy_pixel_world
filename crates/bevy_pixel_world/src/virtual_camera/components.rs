//! Components for the virtual camera system.

use bevy::prelude::*;

/// Marker for virtual camera entities.
///
/// Systems spawn these to request camera control. The virtual camera with
/// the highest priority controls the real camera. Position this entity
/// where you want the camera to look.
///
/// # Priority Conventions
///
/// | Priority | Use Case |
/// |----------|----------|
/// | 0 | Player follow (default) |
/// | 50 | Cutscenes, scripted sequences |
/// | 100 | Debug controller |
/// | 200+ | Console override |
#[derive(Component, Default)]
pub struct VirtualCamera {
  /// Higher priority wins. Equal priority: prefer current active, then lowest
  /// Entity.
  pub priority: i32,
}

impl VirtualCamera {
  /// Create a virtual camera with the given priority.
  pub fn new(priority: i32) -> Self {
    Self { priority }
  }

  /// Default player camera priority.
  pub const PRIORITY_PLAYER: i32 = 0;

  /// Cutscene/scripted sequence priority.
  pub const PRIORITY_CUTSCENE: i32 = 50;

  /// Debug controller priority.
  pub const PRIORITY_DEBUG: i32 = 100;

  /// Console override priority.
  pub const PRIORITY_CONSOLE: i32 = 200;
}
