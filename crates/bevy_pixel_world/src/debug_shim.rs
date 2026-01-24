//! Feature-gated debug gizmo abstraction.
//!
//! Provides a unified interface for emitting debug gizmos that compiles to
//! no-ops when the `visual-debug` feature is disabled.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::coords::{ChunkPos, TilePos, WorldRect};

/// Debug gizmos parameter type.
///
/// When `visual-debug` is enabled, this is `Option<&PendingDebugGizmos>`.
/// When disabled, this is `()` and all emit functions are no-ops.
#[cfg(feature = "visual-debug")]
pub type DebugGizmos<'a> = Option<&'a crate::visual_debug::PendingDebugGizmos>;

#[cfg(not(feature = "visual-debug"))]
pub type DebugGizmos<'a> = ();

/// System parameter for extracting debug gizmos resource.
///
/// Provides a unified interface for systems that need gizmos.
/// When `visual-debug` is enabled, wraps the resource; otherwise a no-op.
#[cfg(feature = "visual-debug")]
#[derive(SystemParam)]
pub struct GizmosParam<'w>(Res<'w, crate::visual_debug::PendingDebugGizmos>);

#[cfg(feature = "visual-debug")]
impl GizmosParam<'_> {
  /// Extracts gizmos as `DebugGizmos` for passing to functions.
  pub fn get(&self) -> DebugGizmos<'_> {
    Some(&*self.0)
  }
}

#[cfg(not(feature = "visual-debug"))]
#[derive(SystemParam)]
pub struct GizmosParam;

#[cfg(not(feature = "visual-debug"))]
impl GizmosParam {
  /// Returns unit type when visual-debug is disabled.
  pub fn get(&self) -> DebugGizmos<'static> {
    ()
  }
}

/// Emit a chunk dirty gizmo.
#[inline]
pub fn emit_chunk(gizmos: DebugGizmos<'_>, pos: ChunkPos) {
  #[cfg(feature = "visual-debug")]
  if let Some(g) = gizmos {
    g.push(crate::visual_debug::PendingGizmo::chunk(pos));
  }
  let _ = (gizmos, pos);
}

/// Emit a tile dirty gizmo.
#[inline]
pub fn emit_tile(gizmos: DebugGizmos<'_>, pos: TilePos) {
  #[cfg(feature = "visual-debug")]
  if let Some(g) = gizmos {
    g.push(crate::visual_debug::PendingGizmo::tile(pos));
  }
  let _ = (gizmos, pos);
}

/// Emit a blit rect gizmo.
#[inline]
pub fn emit_blit_rect(gizmos: DebugGizmos<'_>, rect: WorldRect) {
  #[cfg(feature = "visual-debug")]
  if let Some(g) = gizmos {
    g.push(crate::visual_debug::PendingGizmo::blit_rect(rect));
  }
  let _ = (gizmos, rect);
}

/// Emit a dirty rect gizmo.
#[inline]
pub fn emit_dirty_rect(gizmos: DebugGizmos<'_>, tile: TilePos, bounds: (u8, u8, u8, u8)) {
  #[cfg(feature = "visual-debug")]
  if let Some(g) = gizmos {
    g.push(crate::visual_debug::PendingGizmo::dirty_rect(tile, bounds));
  }
  let _ = (gizmos, tile, bounds);
}
