//! Chunk seeding - populating chunk buffers with initial data.
//!
//! The [`ChunkSeeder`] trait provides a pluggable interface for generating
//! initial pixel data when chunks enter the streaming window.
//!
//! See `docs/architecture/chunk-seeding.md` for the seeder trait design.

mod noise;
pub(crate) mod sdf;

use std::sync::Arc;

use bevy::log::warn;
pub use noise::{MaterialSeeder, NoiseSeeder};

use crate::persistence::{LoadedChunk, WorldSave};
use crate::{Chunk, ChunkPos};

/// Trait for populating chunk buffers with initial data.
///
/// Implementations may generate procedural content ([`NoiseSeeder`]) or
/// load persisted data from disk.
///
/// The `Send + Sync` bounds enable async seeding on background threads.
pub trait ChunkSeeder: Send + Sync {
  /// Fills the chunk buffer with data for the given world position.
  fn seed(&self, pos: ChunkPos, chunk: &mut Chunk);

  /// Fills the chunk buffer with data, optionally using pre-loaded persistence
  /// data.
  ///
  /// Default implementation ignores loaded data and calls `seed()`.
  /// `PersistenceSeeder` overrides this to apply loaded data.
  fn seed_with_loaded(&self, pos: ChunkPos, chunk: &mut Chunk, _loaded: Option<LoadedChunk>) {
    self.seed(pos, chunk);
  }
}

/// Seeder wrapper that applies pre-loaded persistence data.
///
/// Unlike the blocking version, this seeder expects data to be pre-loaded
/// by the async load system. When seeding a chunk:
/// 1. If loaded data is provided and is delta, run inner seeder first
/// 2. Apply loaded data (delta or full)
/// 3. If no loaded data, delegate to inner procedural seeder
pub struct PersistenceSeeder<S: ChunkSeeder> {
  /// Inner procedural seeder (fallback for unpersisted chunks).
  inner: S,
  /// World save file handle (for sync I/O fallback).
  save: Arc<std::sync::RwLock<WorldSave>>,
}

impl<S: ChunkSeeder> PersistenceSeeder<S> {
  /// Creates a new persistent seeder wrapping the given inner seeder.
  pub fn new(inner: S, save: Arc<std::sync::RwLock<WorldSave>>) -> Self {
    Self { inner, save }
  }

  /// Returns a reference to the inner seeder.
  pub fn inner(&self) -> &S {
    &self.inner
  }

  /// Returns a reference to the world save for async loading.
  pub fn save(&self) -> &Arc<std::sync::RwLock<WorldSave>> {
    &self.save
  }

  /// Applies loaded chunk data, handling delta encoding.
  fn apply_loaded_chunk(&self, pos: ChunkPos, chunk: &mut Chunk, loaded_chunk: LoadedChunk) {
    if loaded_chunk.seeder_needed {
      // Delta encoding - need to seed first, then apply delta
      self.inner.seed(pos, chunk);
    }

    if let Err(e) = loaded_chunk.apply_to(chunk) {
      warn!(
        "Failed to apply saved chunk at {:?}: {}. Regenerating.",
        pos, e
      );
      self.inner.seed(pos, chunk);
    } else {
      // Successfully loaded from disk - mark as persisted
      chunk.from_persistence = true;
    }
  }
}

impl<S: ChunkSeeder> ChunkSeeder for PersistenceSeeder<S> {
  fn seed(&self, pos: ChunkPos, chunk: &mut Chunk) {
    // Synchronous fallback: load from save file using block_on
    let loaded = {
      let save = match self.save.read() {
        Ok(s) => s,
        Err(_) => {
          // Lock poisoned, fall back to procedural
          self.inner.seed(pos, chunk);
          return;
        }
      };

      save.load_chunk(pos, &self.inner)
    };

    self.seed_with_loaded(pos, chunk, loaded);
  }

  fn seed_with_loaded(&self, pos: ChunkPos, chunk: &mut Chunk, loaded: Option<LoadedChunk>) {
    // If pre-loaded data is available, use it directly
    if let Some(loaded_chunk) = loaded {
      self.apply_loaded_chunk(pos, chunk, loaded_chunk);
      return;
    }

    // No pre-loaded data provided. This can happen when:
    // 1. The async load system isn't being used (WASM without persistence init)
    // 2. Race condition: save completed after load task was spawned
    // 3. The chunk genuinely isn't in the save file
    //
    // Fall back to synchronous loading to handle cases 1 and 2.
    let sync_loaded = {
      let save = match self.save.read() {
        Ok(s) => s,
        Err(_) => {
          // Lock poisoned, fall back to procedural
          self.inner.seed(pos, chunk);
          return;
        }
      };
      save.load_chunk(pos, &self.inner)
    };

    if let Some(loaded_chunk) = sync_loaded {
      self.apply_loaded_chunk(pos, chunk, loaded_chunk);
    } else {
      // Not in save file, use procedural generation
      self.inner.seed(pos, chunk);
    }
  }
}
