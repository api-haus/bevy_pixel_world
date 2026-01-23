//! Streaming window for chunk management.
//!
//! Manages a rectangular window of active chunks centered on the camera.
//! As the camera moves, chunks are released and acquired from the pool.
//!
//! See `docs/architecture/streaming-window.md` for the active region concept.

use std::collections::HashMap;

use bevy::prelude::*;

use super::pool::{ChunkPool, PoolHandle};
use crate::coords::{ChunkPos, WINDOW_HEIGHT, WINDOW_WIDTH};
use crate::Chunk;

/// An active chunk in the streaming window.
pub struct ActiveChunk {
  /// Handle to the chunk data in the pool.
  pub handle: PoolHandle,
  /// Entity for the chunk's mesh.
  pub entity: Entity,
  /// Handle to the chunk's texture.
  pub texture: Handle<Image>,
  /// Whether the chunk needs GPU upload.
  pub dirty: bool,
}

/// Changes to apply after updating the window center.
pub struct WindowDelta {
  /// Chunks that left the window (position, entity to despawn).
  pub to_despawn: Vec<(ChunkPos, Entity)>,
  /// Chunks that entered the window (need to spawn).
  pub to_spawn: Vec<ChunkPos>,
}

/// Manages streaming chunks around the camera.
#[derive(Resource)]
pub struct StreamingWindow {
  /// Current center of the window in chunk coordinates.
  pub center: ChunkPos,
  /// Active chunks indexed by position.
  pub active: HashMap<ChunkPos, ActiveChunk>,
  /// Pool of pre-allocated chunk memory.
  pub pool: ChunkPool,
}

impl Default for StreamingWindow {
  fn default() -> Self {
    Self::new()
  }
}

impl StreamingWindow {
  /// Creates a new streaming window.
  pub fn new() -> Self {
    Self {
      center: ChunkPos(0, 0),
      active: HashMap::new(),
      pool: ChunkPool::new(),
    }
  }

  /// Computes the set of chunk positions visible in the window.
  fn visible_set(&self) -> impl Iterator<Item = ChunkPos> {
    let cx = self.center.0;
    let cy = self.center.1;
    let hw = WINDOW_WIDTH as i32 / 2;
    let hh = WINDOW_HEIGHT as i32 / 2;

    // 6x4 window spans [center.x - 3, center.x + 2] x [center.y - 2, center.y + 1]
    let x_range = (cx - hw)..(cx + hw);
    let y_range = (cy - hh)..(cy + hh);

    x_range.flat_map(move |x| y_range.clone().map(move |y| ChunkPos(x, y)))
  }

  /// Updates the window center, returning chunks to despawn and spawn.
  ///
  /// Call this when the camera crosses a chunk boundary.
  pub fn update_center(&mut self, new_center: ChunkPos) -> WindowDelta {
    if new_center == self.center {
      return WindowDelta {
        to_despawn: vec![],
        to_spawn: vec![],
      };
    }

    // Compute old and new visible sets
    let old_set: std::collections::HashSet<_> = self.visible_set().collect();

    self.center = new_center;
    let new_set: std::collections::HashSet<_> = self.visible_set().collect();

    // Find chunks to release (in old but not new)
    let mut to_despawn = Vec::new();
    for pos in old_set.difference(&new_set) {
      if let Some(active) = self.active.remove(pos) {
        self.pool.release(active.handle);
        to_despawn.push((*pos, active.entity));
      }
    }

    // Find chunks to acquire (in new but not old)
    let to_spawn: Vec<_> = new_set.difference(&old_set).copied().collect();

    WindowDelta {
      to_despawn,
      to_spawn,
    }
  }

  /// Marks a chunk as needing GPU upload.
  pub fn mark_dirty(&mut self, pos: ChunkPos) {
    if let Some(active) = self.active.get_mut(&pos) {
      active.dirty = true;
    }
  }

  /// Returns a mutable reference to chunk data at the given position.
  pub fn get_chunk_mut(&mut self, pos: ChunkPos) -> Option<&mut Chunk> {
    self.active.get(&pos).map(|a| self.pool.get_mut(a.handle))
  }

  /// Acquires a chunk from the pool.
  ///
  /// Returns `None` if the pool is exhausted. The caller is responsible
  /// for seeding the chunk via a [`ChunkSeeder`](crate::ChunkSeeder).
  pub fn acquire_chunk(&mut self) -> Option<PoolHandle> {
    self.pool.acquire()
  }

  /// Registers an active chunk after spawning its entity.
  pub fn register_active(
    &mut self,
    pos: ChunkPos,
    handle: PoolHandle,
    entity: Entity,
    texture: Handle<Image>,
  ) {
    self.active.insert(
      pos,
      ActiveChunk {
        handle,
        entity,
        texture,
        dirty: true,
      },
    );
  }
}
