//! Chunk slot management for the pooled chunk storage.

use bevy::prelude::*;

use crate::coords::CHUNK_SIZE;
use crate::primitives::Chunk;
use crate::render::ChunkMaterial;

/// Lifecycle state of a chunk slot.
///
/// Tracks the slot's position in the pooling state machine:
/// `InPool` → `Seeding` → `Active` → `Recycling` → `InPool`
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChunkLifecycle {
  /// Slot is in the pool, available for allocation.
  #[default]
  InPool,
  /// Slot has been assigned a position but is awaiting seed data.
  Seeding,
  /// Slot is fully active with valid pixel data.
  Active,
  /// Slot is being recycled back to the pool.
  Recycling,
}

/// Index into the PixelWorld's fixed-size slot array.
///
/// SlotIndex provides stable identity for a chunk's storage location,
/// independent of the world position assigned to that slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SlotIndex(pub usize);

/// A slot in the chunk storage.
///
/// Each slot contains pre-allocated chunk memory and lifecycle state.
pub struct ChunkSlot {
  /// Pre-allocated chunk memory.
  pub chunk: Chunk,
  /// Current lifecycle state of this slot.
  pub lifecycle: ChunkLifecycle,
  /// World position if active, None if in pool.
  pub pos: Option<crate::coords::ChunkPos>,
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
      lifecycle: ChunkLifecycle::InPool,
      pos: None,
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
    self.lifecycle == ChunkLifecycle::InPool
  }

  /// Returns true if the chunk has valid pixel data for its position.
  /// Derived from lifecycle state: true when Active.
  #[inline]
  pub fn is_seeded(&self) -> bool {
    self.lifecycle == ChunkLifecycle::Active
  }

  /// Initializes the slot for a new chunk position.
  ///
  /// Transitions from InPool to Seeding state and prepares for seeding.
  pub(crate) fn initialize(&mut self, pos: crate::coords::ChunkPos) {
    self.lifecycle = ChunkLifecycle::Seeding;
    self.pos = Some(pos);
    self.chunk.set_pos(pos);
    self.dirty = false;
    self.modified = false;
    self.persisted = false;
  }

  /// Resets the slot to pool state.
  ///
  /// Returns true if the chunk needs saving (was dirty but not persisted).
  pub(crate) fn release(&mut self) -> bool {
    let needs_save = self.needs_save();
    self.chunk.clear_pos();
    self.lifecycle = ChunkLifecycle::InPool;
    self.pos = None;
    self.dirty = false;
    self.modified = false;
    self.persisted = false;
    self.entity = None;
    // Keep texture and material handles - they'll be reused
    needs_save
  }

  /// Returns true if the chunk has modifications that need saving.
  pub fn needs_save(&self) -> bool {
    self.is_seeded() && self.modified && !self.persisted
  }

  /// Marks the chunk as persisted to disk.
  pub fn mark_persisted(&mut self) {
    self.persisted = true;
  }
}
