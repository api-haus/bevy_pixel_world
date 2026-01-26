//! Frame reset systems for streaming state.
//!
//! Clears per-frame tracking resources at the start of each frame.

use bevy::prelude::*;

use super::{SeededChunks, UnloadingChunks};

/// System: Clears chunk tracking resources at the start of each frame.
pub(crate) fn clear_chunk_tracking(
  mut unloading: ResMut<UnloadingChunks>,
  mut seeded: ResMut<SeededChunks>,
) {
  unloading.positions.clear();
  seeded.positions.clear();
}
