//! Streaming window logic for chunk visibility.

use std::collections::HashSet;

use bevy::prelude::*;

use super::slot::SlotIndex;
use crate::coords::{ChunkPos, WINDOW_HEIGHT, WINDOW_WIDTH};

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

impl StreamingDelta {
  /// Returns an empty delta (no changes).
  pub fn empty() -> Self {
    Self {
      to_despawn: Vec::new(),
      to_spawn: Vec::new(),
      to_save: Vec::new(),
    }
  }
}

/// Computes which chunk positions are leaving and entering the streaming
/// window.
///
/// Returns (positions_leaving, positions_entering).
pub(crate) fn compute_position_changes(
  old_center: ChunkPos,
  new_center: ChunkPos,
) -> (Vec<ChunkPos>, Vec<ChunkPos>) {
  let old_set: HashSet<_> = visible_positions(old_center).collect();
  let new_set: HashSet<_> = visible_positions(new_center).collect();

  let leaving: Vec<_> = old_set.difference(&new_set).copied().collect();
  let entering: Vec<_> = new_set.difference(&old_set).copied().collect();

  (leaving, entering)
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
