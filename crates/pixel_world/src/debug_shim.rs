//! Feature-gated debug gizmo abstraction.
//!
//! Provides a unified interface for emitting debug gizmos that compiles to
//! no-ops when the `visual-debug` feature is disabled.

use crate::coords::{ChunkPos, TilePos, WorldRect};

/// Debug gizmos parameter type.
///
/// When `visual-debug` is enabled, this is `Option<&PendingDebugGizmos>`.
/// When disabled, this is `()` and all emit functions are no-ops.
#[cfg(feature = "visual-debug")]
pub type DebugGizmos<'a> = Option<&'a crate::visual_debug::PendingDebugGizmos>;

#[cfg(not(feature = "visual-debug"))]
pub type DebugGizmos<'a> = ();

/// Emit a chunk dirty gizmo.
#[cfg(feature = "visual-debug")]
#[inline]
pub fn emit_chunk(gizmos: DebugGizmos<'_>, pos: ChunkPos) {
    if let Some(g) = gizmos {
        g.push(crate::visual_debug::PendingGizmo::chunk(pos));
    }
}

#[cfg(not(feature = "visual-debug"))]
#[inline]
pub fn emit_chunk(_: DebugGizmos<'_>, _: ChunkPos) {}

/// Emit a tile dirty gizmo.
#[cfg(feature = "visual-debug")]
#[inline]
pub fn emit_tile(gizmos: DebugGizmos<'_>, pos: TilePos) {
    if let Some(g) = gizmos {
        g.push(crate::visual_debug::PendingGizmo::tile(pos));
    }
}

#[cfg(not(feature = "visual-debug"))]
#[inline]
pub fn emit_tile(_: DebugGizmos<'_>, _: TilePos) {}

/// Emit a blit rect gizmo.
#[cfg(feature = "visual-debug")]
#[inline]
pub fn emit_blit_rect(gizmos: DebugGizmos<'_>, rect: WorldRect) {
    if let Some(g) = gizmos {
        g.push(crate::visual_debug::PendingGizmo::blit_rect(rect));
    }
}

#[cfg(not(feature = "visual-debug"))]
#[inline]
pub fn emit_blit_rect(_: DebugGizmos<'_>, _: WorldRect) {}
