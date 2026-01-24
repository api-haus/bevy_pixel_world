//! Streaming window logic for chunk visibility.

use bevy::prelude::*;

use crate::coords::{ChunkPos, WINDOW_HEIGHT, WINDOW_WIDTH};

use super::slot::SlotIndex;

/// Changes from updating the streaming window center.
pub(crate) struct StreamingDelta {
  /// Chunks that left the window (position, entity to despawn).
  pub to_despawn: Vec<(ChunkPos, Entity)>,
  /// Chunks that entered the window (position, slot index).
  pub to_spawn: Vec<(ChunkPos, SlotIndex)>,
  /// Chunks that need saving before being released (position, raw pixel data).
  /// The pixel data is cloned before the slot is released.
  pub to_save: Vec<ChunkSaveData>,
}

/// Data needed to save a chunk that's leaving the streaming window.
pub struct ChunkSaveData {
  /// Chunk position.
  pub pos: ChunkPos,
  /// Raw pixel bytes (will be compressed by persistence system).
  pub pixels: Vec<u8>,
}

/// Returns iterator over visible chunk positions for a given center.
pub(crate) fn visible_positions(center: ChunkPos) -> impl Iterator<Item = ChunkPos> {
  let cx = center.x;
  let cy = center.y;
  let hw = WINDOW_WIDTH as i32 / 2;
  let hh = WINDOW_HEIGHT as i32 / 2;

  let x_range = (cx - hw)..(cx + hw);
  let y_range = (cy - hh)..(cy + hh);

  x_range.flat_map(move |x| y_range.clone().map(move |y| ChunkPos::new(x, y)))
}
