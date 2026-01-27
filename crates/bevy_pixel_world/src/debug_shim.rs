//! Debug gizmo abstraction.
//!
//! Provides a unified interface for emitting debug gizmos.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::coords::{ChunkPos, TilePos, WorldRect};

/// Debug gizmos handle for passing to emit functions.
///
/// Wraps `Option<&PendingDebugGizmos>`.
#[derive(Clone, Copy, Default)]
pub struct DebugGizmos<'a>(Option<&'a crate::visual_debug::PendingDebugGizmos>);

impl DebugGizmos<'_> {
  /// Creates a no-op gizmos handle.
  ///
  /// Useful in tests and contexts without visual debug infrastructure.
  #[inline]
  pub fn none() -> Self {
    DebugGizmos(None)
  }
}

/// System parameter for extracting debug gizmos resource.
///
/// Provides a unified interface for systems that need gizmos.
/// Returns a no-op handle when `PendingDebugGizmos` is not available
/// (e.g. in headless mode).
#[derive(SystemParam)]
pub struct GizmosParam<'w> {
  inner: Option<Res<'w, crate::visual_debug::PendingDebugGizmos>>,
}

impl GizmosParam<'_> {
  /// Extracts gizmos as `DebugGizmos` for passing to functions.
  pub fn get(&self) -> DebugGizmos<'_> {
    match &self.inner {
      Some(res) => DebugGizmos(Some(&*res)),
      None => DebugGizmos(None),
    }
  }
}

/// Emit a chunk dirty gizmo.
#[inline]
pub fn emit_chunk(gizmos: DebugGizmos<'_>, pos: ChunkPos) {
  if let Some(g) = gizmos.0 {
    g.push(crate::visual_debug::PendingGizmo::chunk(pos));
  }
  let _ = (gizmos, pos);
}

/// Emit a tile dirty gizmo.
#[inline]
pub fn emit_tile(gizmos: DebugGizmos<'_>, pos: TilePos) {
  if let Some(g) = gizmos.0 {
    g.push(crate::visual_debug::PendingGizmo::tile(pos));
  }
  let _ = (gizmos, pos);
}

/// Emit a blit rect gizmo.
#[inline]
pub fn emit_blit_rect(gizmos: DebugGizmos<'_>, rect: WorldRect) {
  if let Some(g) = gizmos.0 {
    g.push(crate::visual_debug::PendingGizmo::blit_rect(rect));
  }
  let _ = (gizmos, rect);
}

/// Emit a dirty rect gizmo.
#[inline]
pub fn emit_dirty_rect(gizmos: DebugGizmos<'_>, tile: TilePos, bounds: (u8, u8, u8, u8)) {
  if let Some(g) = gizmos.0 {
    g.push(crate::visual_debug::PendingGizmo::dirty_rect(tile, bounds));
  }
  let _ = (gizmos, tile, bounds);
}
