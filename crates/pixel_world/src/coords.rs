//! Coordinate types and spatial constants.
//!
//! Defines the coordinate system for the world:
//! - [`WorldPos`]: Absolute pixel position (i64 for infinite worlds)
//! - [`ChunkPos`]: Chunk grid position (i32)
//! - [`LocalPos`]: Position within a chunk (u16)
//! - [`MaterialId`]: Material registry index (0-255)
//! - [`ColorIndex`]: Palette color index (0-255)

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

    // Floor division: for negative numbers, we need to round toward negative
    // infinity
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

/// Material registry index (0-255).
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct MaterialId(pub u8);

/// Palette color index (0-255).
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct ColorIndex(pub u8);

/// Tile position in the world grid.
///
/// Each tile spans [`TILE_SIZE`] pixels in each dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TilePos(pub i64, pub i64);

/// World-coordinate axis-aligned bounding box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldRect {
  pub x: i64,
  pub y: i64,
  pub width: u32,
  pub height: u32,
}

impl WorldRect {
  /// Creates a new world rectangle.
  pub const fn new(x: i64, y: i64, width: u32, height: u32) -> Self {
    Self {
      x,
      y,
      width,
      height,
    }
  }

  /// Creates a rectangle centered on a point with given radius.
  pub fn centered(center_x: i64, center_y: i64, radius: u32) -> Self {
    let diameter = radius * 2 + 1;
    Self {
      x: center_x - radius as i64,
      y: center_y - radius as i64,
      width: diameter,
      height: diameter,
    }
  }

  /// Returns true if the given world position is within this rect.
  pub fn contains(&self, pos: WorldPos) -> bool {
    pos.0 >= self.x
      && pos.0 < self.x + self.width as i64
      && pos.1 >= self.y
      && pos.1 < self.y + self.height as i64
  }

  /// Returns the range of tile positions that overlap this rect.
  pub fn to_tile_range(&self) -> impl Iterator<Item = TilePos> {
    let tile_size = TILE_SIZE as i64;

    // Compute inclusive tile bounds using floor division
    let min_tx = self.x.div_euclid(tile_size);
    let min_ty = self.y.div_euclid(tile_size);
    let max_tx = (self.x + self.width as i64 - 1).div_euclid(tile_size);
    let max_ty = (self.y + self.height as i64 - 1).div_euclid(tile_size);

    (min_tx..=max_tx).flat_map(move |tx| (min_ty..=max_ty).map(move |ty| TilePos(tx, ty)))
  }
}

/// Fragment data for world-space blitting.
///
/// Passed to blit callbacks with both absolute position and normalized coords.
#[derive(Clone, Copy, Debug)]
pub struct WorldFragment {
  /// Absolute X coordinate in world space.
  pub x: i64,
  /// Absolute Y coordinate in world space (Y+ up).
  pub y: i64,
  /// Normalized U coordinate within the blit rect (0.0 at left, 1.0 at right).
  pub u: f32,
  /// Normalized V coordinate within the blit rect (0.0 at bottom, 1.0 at top).
  pub v: f32,
}
