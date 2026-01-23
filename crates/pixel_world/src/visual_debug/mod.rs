//! Visual debug suite for pixel_world.
//!
//! Provides debug gizmo rendering for chunk updates, tile updates, and blit
//! operations. Enable with the `visual-debug` feature flag.

pub(super) mod colors;
mod gizmos;
mod systems;

use bevy::prelude::*;
pub use gizmos::{ActiveGizmos, GizmoKind, PendingDebugGizmos, PendingGizmo};
use systems::render_debug_gizmos;

/// Plugin that enables visual debug gizmos.
pub struct VisualDebugPlugin;

impl Plugin for VisualDebugPlugin {
  fn build(&self, app: &mut App) {
    app
      .init_resource::<PendingDebugGizmos>()
      .init_resource::<ActiveGizmos>()
      .add_systems(Update, render_debug_gizmos);
  }
}
