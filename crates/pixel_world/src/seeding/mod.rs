//! Chunk seeding - populating chunk buffers with initial data.
//!
//! The [`ChunkSeeder`] trait provides a pluggable interface for generating
//! initial pixel data when chunks enter the streaming window.
//!
//! See `docs/architecture/chunk-seeding.md` for the seeder trait design.

mod noise;
pub mod sdf;

pub use noise::{MaterialSeeder, NoiseSeeder};

use crate::{Chunk, ChunkPos};

/// Trait for populating chunk buffers with initial data.
///
/// Implementations may generate procedural content ([`NoiseSeeder`]) or
/// load persisted data from disk.
pub trait ChunkSeeder {
  /// Fills the chunk buffer with data for the given world position.
  fn seed(&self, pos: ChunkPos, chunk: &mut Chunk);
}
