//! Chunk slot management for the pooled chunk storage.

use bevy::prelude::*;

use crate::coords::CHUNK_SIZE;
use crate::primitives::Chunk;
use crate::render::ChunkMaterial;

/// Index into the slots array.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SlotIndex(pub usize);

/// A slot in the chunk storage.
///
/// Each slot contains pre-allocated chunk memory and lifecycle state.
pub struct ChunkSlot {
  /// Pre-allocated chunk memory.
  pub chunk: Chunk,
  /// World position if active, None if in pool.
  pub pos: Option<crate::coords::ChunkPos>,
  /// Whether the chunk has valid pixel data for its position.
  /// When false, the chunk needs seeding before it can participate in
  /// simulation.
  pub seeded: bool,
  /// Whether the chunk's CPU data differs from GPU texture.
  /// When true, the chunk needs upload.
  pub dirty: bool,
  /// Entity displaying this chunk (when active).
  pub entity: Option<Entity>,
  /// Texture handle for GPU upload.
  pub texture: Option<Handle<Image>>,
  /// Material handle (for bind group refresh workaround).
  pub material: Option<Handle<ChunkMaterial>>,
}

impl ChunkSlot {
  /// Creates a new slot with pre-allocated chunk memory.
  pub(crate) fn new() -> Self {
    Self {
      chunk: Chunk::new(CHUNK_SIZE, CHUNK_SIZE),
      pos: None,
      seeded: false,
      dirty: false,
      entity: None,
      texture: None,
      material: None,
    }
  }

  /// Returns true if this slot is available for use.
  pub(crate) fn is_free(&self) -> bool {
    self.pos.is_none()
  }

  /// Resets the slot to pool state.
  pub(crate) fn release(&mut self) {
    self.chunk.clear_pos();
    self.pos = None;
    self.seeded = false;
    self.dirty = false;
    self.entity = None;
    // Keep texture and material handles - they'll be reused
  }
}
