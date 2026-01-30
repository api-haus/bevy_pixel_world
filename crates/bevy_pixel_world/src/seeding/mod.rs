//! Chunk seeding - populating chunk buffers with initial data.
//!
//! The [`ChunkSeeder`] trait provides a pluggable interface for generating
//! initial pixel data when chunks enter the streaming window.
//!
//! See `docs/architecture/chunk-seeding.md` for the seeder trait design.

mod noise;
pub(crate) mod sdf;

pub use noise::{MaterialSeeder, NoiseSeeder, presets};

use crate::persistence::LoadedChunk;
use crate::{Chunk, ChunkPos};

/// Trait for populating chunk buffers with initial data.
///
/// Implementations generate procedural content ([`NoiseSeeder`],
/// [`MaterialSeeder`]). Persistence loading is handled separately by the
/// streaming system (`dispatch_chunk_loads` and `seed_chunk_with_loaded`).
///
/// The `Send + Sync` bounds enable async seeding on background threads.
pub trait ChunkSeeder: Send + Sync {
  /// Fills the chunk buffer with data for the given world position.
  fn seed(&self, pos: ChunkPos, chunk: &mut Chunk);

  /// Fills the chunk buffer with data, optionally using pre-loaded
  /// persistence data.
  ///
  /// Default implementation ignores loaded data and calls `seed()`.
  /// Override this to handle persistence data integration.
  fn seed_with_loaded(&self, pos: ChunkPos, chunk: &mut Chunk, _loaded: Option<LoadedChunk>) {
    self.seed(pos, chunk);
  }
}
