//! Visual debug suite for pixel_world.
//!
//! Provides debug gizmo rendering for chunk updates, tile updates, and blit
//! operations. Enable with the `visual-debug` feature flag.

pub(super) mod colors;
mod gizmos;
pub mod persistence;
pub mod settings;
mod systems;
mod ui;

use bevy::prelude::*;
pub use gizmos::{ActiveGizmos, GizmoKind, PendingDebugGizmos, PendingGizmo};
pub use persistence::SettingsPersistence;
pub use settings::VisualDebugSettings;
use systems::{draw_pixel_body_centers, render_debug_gizmos};
pub use ui::visual_debug_checkboxes;

/// Plugin that enables visual debug gizmos.
pub struct VisualDebugPlugin;

impl Plugin for VisualDebugPlugin {
  fn build(&self, app: &mut App) {
    app
      .init_resource::<PendingDebugGizmos>()
      .init_resource::<ActiveGizmos>()
      .add_systems(Startup, persistence::load_settings)
      .add_systems(Update, (render_debug_gizmos, draw_pixel_body_centers))
      .add_systems(
        Update,
        (persistence::save_settings, systems::sync_collision_config)
          .run_if(resource_exists::<VisualDebugSettings>),
      );
  }
}
