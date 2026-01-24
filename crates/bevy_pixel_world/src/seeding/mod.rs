//! Chunk seeding - populating chunk buffers with initial data.
//!
//! The [`ChunkSeeder`] trait provides a pluggable interface for generating
//! initial pixel data when chunks enter the streaming window.
//!
//! See `docs/architecture/chunk-seeding.md` for the seeder trait design.

mod noise;
pub(crate) mod sdf;

pub use noise::{MaterialSeeder, NoiseSeeder};

use std::sync::Arc;

use crate::persistence::WorldSave;
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
}

/// Seeder wrapper that checks persistence before procedural generation.
///
/// When seeding a chunk:
/// 1. Check if the chunk exists in the save file
/// 2. If found, load from disk (applying delta if needed)
/// 3. If not found, delegate to the inner procedural seeder
pub struct PersistenceSeeder<S: ChunkSeeder> {
  /// Inner procedural seeder (fallback for unpersisted chunks).
  inner: S,
  /// World save file handle.
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
}

impl<S: ChunkSeeder> ChunkSeeder for PersistenceSeeder<S> {
  fn seed(&self, pos: ChunkPos, chunk: &mut Chunk) {
    // Try to load from save file
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

    match loaded {
      Some(loaded_chunk) => {
        // Found in save file
        if loaded_chunk.seeder_needed {
          // Delta encoding - need to seed first, then apply delta
          self.inner.seed(pos, chunk);
        }

        if let Err(e) = loaded_chunk.apply_to(chunk) {
          eprintln!(
            "Warning: failed to apply saved chunk at {:?}: {}. Regenerating.",
            pos, e
          );
          self.inner.seed(pos, chunk);
        } else {
          // Successfully loaded from disk - mark as persisted
          chunk.from_persistence = true;
        }
      }
      None => {
        // Not persisted, use procedural generation
        self.inner.seed(pos, chunk);
      }
    }
  }
}
