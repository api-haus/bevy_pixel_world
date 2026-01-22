//! Coordinate types and spatial constants.
//!
//! Defines the coordinate system for the world:
//! - [`WorldPos`]: Absolute pixel position (i64 for infinite worlds)
//! - [`ChunkPos`]: Chunk grid position (i32)
//! - [`LocalPos`]: Position within a chunk (u16)

/// Size of a chunk in pixels (width and height).
pub const CHUNK_SIZE: u32 = 512;

/// Size of a tile in pixels.
pub const TILE_SIZE: u32 = 16;

/// Width of the streaming window in chunks.
pub const WINDOW_WIDTH: u32 = 6;

/// Height of the streaming window in chunks.
pub const WINDOW_HEIGHT: u32 = 4;

/// Number of chunks in the pool (derived from window size).
pub const POOL_SIZE: usize = (WINDOW_WIDTH * WINDOW_HEIGHT) as usize;

/// Number of tiles per chunk edge (derived from chunk/tile sizes).
pub const TILES_PER_CHUNK: u32 = CHUNK_SIZE / TILE_SIZE;

/// Absolute pixel position in the world.
///
/// Uses i64 for effectively infinite worlds without overflow concerns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WorldPos(pub i64, pub i64);

/// Position in the chunk grid.
///
/// Each chunk spans [`CHUNK_SIZE`] pixels in each dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChunkPos(pub i32, pub i32);

/// Position within a chunk (0 to CHUNK_SIZE-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalPos(pub u16, pub u16);

impl WorldPos {
  /// Convert to chunk position and local offset.
  ///
  /// Uses floor division for correct negative coordinate handling.
  /// For example, world position -1 maps to chunk -1 with local offset 511.
  pub fn to_chunk_and_local(self) -> (ChunkPos, LocalPos) {
    let chunk_size = CHUNK_SIZE as i64;

    // Floor division: for negative numbers, we need to round toward negative infinity
    let cx = self.0.div_euclid(chunk_size) as i32;
    let cy = self.1.div_euclid(chunk_size) as i32;

    // Local offset is always positive (0 to CHUNK_SIZE-1)
    let lx = self.0.rem_euclid(chunk_size) as u16;
    let ly = self.1.rem_euclid(chunk_size) as u16;

    (ChunkPos(cx, cy), LocalPos(lx, ly))
  }
}

impl ChunkPos {
  /// Convert chunk origin to world position.
  ///
  /// Returns the bottom-left corner of the chunk in world coordinates.
  pub fn to_world(self) -> WorldPos {
    let chunk_size = CHUNK_SIZE as i64;
    WorldPos(self.0 as i64 * chunk_size, self.1 as i64 * chunk_size)
  }
}
