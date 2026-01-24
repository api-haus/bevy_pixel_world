//! Visual debug settings resource.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::gizmos::GizmoKind;

/// Settings for visual debug overlays.
///
/// All visualizations are opt-in (disabled by default).
#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct VisualDebugSettings {
    /// Show collision mesh outlines.
    pub show_collision_meshes: bool,
    /// Show chunk boundary rectangles.
    pub show_chunk_boundaries: bool,
    /// Show tile boundary rectangles.
    pub show_tile_boundaries: bool,
    /// Show dirty rect highlights.
    pub show_dirty_rects: bool,
    /// Show blit rect highlights.
    pub show_blit_rects: bool,
}

impl Default for VisualDebugSettings {
    fn default() -> Self {
        Self {
            show_collision_meshes: false,
            show_chunk_boundaries: false,
            show_tile_boundaries: false,
            show_dirty_rects: false,
            show_blit_rects: false,
        }
    }
}

impl VisualDebugSettings {
    /// Returns whether the given gizmo kind is enabled.
    pub fn is_enabled(&self, kind: GizmoKind) -> bool {
        match kind {
            GizmoKind::Chunk => self.show_chunk_boundaries,
            GizmoKind::Tile => self.show_tile_boundaries,
            GizmoKind::BlitRect => self.show_blit_rects,
            GizmoKind::DirtyRect => self.show_dirty_rects,
        }
    }
}
