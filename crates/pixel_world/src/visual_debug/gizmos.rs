//! Debug gizmo data structures.

use std::sync::Mutex;

use bevy::prelude::*;

use super::colors;
use crate::coords::{ChunkPos, TilePos, WorldRect, CHUNK_SIZE, TILE_SIZE};

/// Kind of debug gizmo with associated duration.
#[derive(Clone, Copy, Debug)]
pub enum GizmoKind {
  /// Chunk update (gold, 0.1s).
  Chunk,
  /// Tile update (purple, 0.1s).
  Tile,
  /// Blit rect (coral, 0.02s).
  BlitRect,
  /// Dirty rect (mint, 1/60s - synced to simulation tick rate).
  DirtyRect,
}

impl GizmoKind {
  /// Duration in seconds before the gizmo expires.
  pub fn duration(self) -> f32 {
    match self {
      GizmoKind::Chunk | GizmoKind::Tile => 0.1,
      GizmoKind::BlitRect => 0.02,
      GizmoKind::DirtyRect => 1.0 / 60.0,
    }
  }

  /// Color for this gizmo kind.
  pub fn color(self) -> Color {
    match self {
      GizmoKind::Chunk => colors::GOLD,
      GizmoKind::Tile => colors::PURPLE,
      GizmoKind::BlitRect => colors::CORAL,
      GizmoKind::DirtyRect => colors::MINT,
    }
  }
}

/// A pending gizmo waiting to be processed by the render system.
#[derive(Clone, Debug)]
pub struct PendingGizmo {
  pub kind: GizmoKind,
  pub rect: WorldRect,
}

impl PendingGizmo {
  /// Creates a gizmo for a chunk position.
  pub fn chunk(pos: ChunkPos) -> Self {
    let world = pos.to_world();
    Self {
      kind: GizmoKind::Chunk,
      rect: WorldRect::new(world.x, world.y, CHUNK_SIZE, CHUNK_SIZE),
    }
  }

  /// Creates a gizmo for a tile position.
  pub fn tile(pos: TilePos) -> Self {
    let tile_size = TILE_SIZE as i64;
    Self {
      kind: GizmoKind::Tile,
      rect: WorldRect::new(pos.x * tile_size, pos.y * tile_size, TILE_SIZE, TILE_SIZE),
    }
  }

  /// Creates a gizmo for a blit rect.
  pub fn blit_rect(rect: WorldRect) -> Self {
    Self {
      kind: GizmoKind::BlitRect,
      rect,
    }
  }

  /// Creates a gizmo for a tile's dirty rect.
  ///
  /// Takes the tile position and the dirty rect bounds (min_x, min_y, max_x,
  /// max_y) relative to the tile origin.
  pub fn dirty_rect(tile: TilePos, bounds: (u8, u8, u8, u8)) -> Self {
    let tile_size = TILE_SIZE as i64;
    let tile_origin_x = tile.x * tile_size;
    let tile_origin_y = tile.y * tile_size;

    let (min_x, min_y, max_x, max_y) = bounds;
    let x = tile_origin_x + min_x as i64;
    let y = tile_origin_y + min_y as i64;
    let width = (max_x - min_x + 1) as u32;
    let height = (max_y - min_y + 1) as u32;

    Self {
      kind: GizmoKind::DirtyRect,
      rect: WorldRect::new(x, y, width, height),
    }
  }
}

/// Thread-safe pending gizmo queue.
///
/// Used to collect gizmos from parallel blit operations.
#[derive(Resource, Default)]
pub struct PendingDebugGizmos {
  pending: Mutex<Vec<PendingGizmo>>,
}

impl PendingDebugGizmos {
  /// Pushes a gizmo to the pending queue.
  pub fn push(&self, gizmo: PendingGizmo) {
    if let Ok(mut pending) = self.pending.lock() {
      pending.push(gizmo);
    }
  }

  /// Drains all pending gizmos.
  pub fn drain(&self) -> Vec<PendingGizmo> {
    if let Ok(mut pending) = self.pending.lock() {
      std::mem::take(&mut *pending)
    } else {
      Vec::new()
    }
  }
}

/// An active gizmo being rendered.
pub struct ActiveGizmo {
  pub kind: GizmoKind,
  pub rect: WorldRect,
  pub spawn_time: f32,
}

/// Collection of active gizmos being rendered.
#[derive(Resource, Default)]
pub struct ActiveGizmos {
  pub gizmos: Vec<ActiveGizmo>,
}
