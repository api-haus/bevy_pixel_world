//! Pixel displacement state tracking for pixel bodies.
//!
//! Displacement is handled during the clear/blit phases. When a body moves:
//! - Clear at position A creates voids
//! - Blit at position B swaps existing pixels into those voids

use bevy::prelude::*;

/// Tracks the previous transform for displacement calculations.
///
/// Added automatically to pixel bodies during spawn. Used to determine
/// movement direction for pixel displacement.
#[derive(Component, Default)]
pub struct DisplacementState {
  /// Transform from the previous frame.
  pub previous_transform: Option<GlobalTransform>,
}
