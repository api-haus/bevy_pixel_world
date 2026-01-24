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
  /// Whether the chunk has been modified by user actions (paint, erase, swap).
  /// Set when modified, cleared when saved to disk.
  pub modified: bool,
  /// Whether the chunk has been persisted to disk since last modification.
  pub persisted: bool,
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
      modified: false,
      persisted: false,
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
  ///
  /// Returns true if the chunk needs saving (was dirty but not persisted).
  pub(crate) fn release(&mut self) -> bool {
    let needs_save = self.needs_save();
    self.chunk.clear_pos();
    self.pos = None;
    self.seeded = false;
    self.dirty = false;
    self.modified = false;
    self.persisted = false;
    self.entity = None;
    // Keep texture and material handles - they'll be reused
    needs_save
  }

  /// Returns true if the chunk has modifications that need saving.
  pub fn needs_save(&self) -> bool {
    self.seeded && self.modified && !self.persisted
  }

  /// Marks the chunk as persisted to disk.
  pub fn mark_persisted(&mut self) {
    self.persisted = true;
  }
}
