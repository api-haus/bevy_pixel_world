//! Feature-gated debug gizmo abstraction.
//!
//! Provides a unified interface for emitting debug gizmos that compiles to
//! no-ops when the `visual_debug` feature is disabled.

#[cfg(not(feature = "visual_debug"))]
use std::marker::PhantomData;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::coords::{ChunkPos, TilePos, WorldRect};

/// Debug gizmos handle for passing to emit functions.
///
/// When `visual_debug` is enabled, wraps `Option<&PendingDebugGizmos>`.
/// When disabled, this is a ZST and all emit functions are no-ops.
#[derive(Clone, Copy, Default)]
pub struct DebugGizmos<'a>(
  #[cfg(feature = "visual_debug")] Option<&'a crate::visual_debug::PendingDebugGizmos>,
  #[cfg(not(feature = "visual_debug"))] PhantomData<&'a ()>,
);

impl DebugGizmos<'_> {
  /// Creates a no-op gizmos handle.
  ///
  /// Useful in tests and contexts without visual debug infrastructure.
  #[inline]
  pub fn none() -> Self {
    #[cfg(feature = "visual_debug")]
    {
      DebugGizmos(None)
    }
    #[cfg(not(feature = "visual_debug"))]
    {
      DebugGizmos(PhantomData)
    }
  }
}

/// System parameter for extracting debug gizmos resource.
///
/// Provides a unified interface for systems that need gizmos.
/// When `visual_debug` is enabled, wraps the resource; otherwise a ZST.
#[derive(SystemParam)]
pub struct GizmosParam<'w> {
  #[cfg(feature = "visual_debug")]
  inner: Res<'w, crate::visual_debug::PendingDebugGizmos>,
  #[cfg(not(feature = "visual_debug"))]
  _marker: PhantomData<&'w ()>,
}

impl GizmosParam<'_> {
  /// Extracts gizmos as `DebugGizmos` for passing to functions.
  pub fn get(&self) -> DebugGizmos<'_> {
    #[cfg(feature = "visual_debug")]
    {
      DebugGizmos(Some(&*self.inner))
    }
    #[cfg(not(feature = "visual_debug"))]
    {
      DebugGizmos(PhantomData)
    }
  }
}

/// Emit a chunk dirty gizmo.
#[inline]
pub fn emit_chunk(gizmos: DebugGizmos<'_>, pos: ChunkPos) {
  #[cfg(feature = "visual_debug")]
  if let Some(g) = gizmos.0 {
    g.push(crate::visual_debug::PendingGizmo::chunk(pos));
  }
  let _ = (gizmos, pos);
}

/// Emit a tile dirty gizmo.
#[inline]
pub fn emit_tile(gizmos: DebugGizmos<'_>, pos: TilePos) {
  #[cfg(feature = "visual_debug")]
  if let Some(g) = gizmos.0 {
    g.push(crate::visual_debug::PendingGizmo::tile(pos));
  }
  let _ = (gizmos, pos);
}

/// Emit a blit rect gizmo.
#[inline]
pub fn emit_blit_rect(gizmos: DebugGizmos<'_>, rect: WorldRect) {
  #[cfg(feature = "visual_debug")]
  if let Some(g) = gizmos.0 {
    g.push(crate::visual_debug::PendingGizmo::blit_rect(rect));
  }
  let _ = (gizmos, rect);
}

/// Emit a dirty rect gizmo.
#[inline]
pub fn emit_dirty_rect(gizmos: DebugGizmos<'_>, tile: TilePos, bounds: (u8, u8, u8, u8)) {
  #[cfg(feature = "visual_debug")]
  if let Some(g) = gizmos.0 {
    g.push(crate::visual_debug::PendingGizmo::dirty_rect(tile, bounds));
  }
  let _ = (gizmos, tile, bounds);
}
