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
pub const TILE_SIZE: u32 = 32;

/// Width of the streaming window in chunks.
pub(crate) const WINDOW_WIDTH: u32 = 4;

/// Height of the streaming window in chunks.
pub(crate) const WINDOW_HEIGHT: u32 = 3;

/// Number of chunks in the pool (derived from window size).
pub(crate) const POOL_SIZE: usize = (WINDOW_WIDTH * WINDOW_HEIGHT) as usize;

/// Number of tiles per chunk edge (derived from chunk/tile sizes).
pub const TILES_PER_CHUNK: u32 = CHUNK_SIZE / TILE_SIZE;

/// Phase assignment for 2x2 checkerboard scheduling.
///
/// Tiles are assigned to phases based on position modulo 2:
/// - A = (0, 1) - top-left of 2x2 block (Y+ up coordinate system)
/// - B = (1, 1) - top-right
/// - C = (0, 0) - bottom-left
/// - D = (1, 0) - bottom-right
///
/// This mapping ensures tiles in the same phase are never adjacent,
/// allowing safe parallel execution within a phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
  A, // (0, 1) - top-left
  B, // (1, 1) - top-right
  C, // (0, 0) - bottom-left
  D, // (1, 0) - bottom-right
}

impl Phase {
  /// Returns the phase for a tile at the given position.
  pub fn from_tile(tile: TilePos) -> Phase {
    let px = tile.x.rem_euclid(2);
    let py = tile.y.rem_euclid(2);
    match (px, py) {
      (0, 1) => Phase::A,
      (1, 1) => Phase::B,
      (0, 0) => Phase::C,
      (1, 0) => Phase::D,
      _ => unreachable!(),
    }
  }

  /// Returns the index (0-3) for this phase.
  pub const fn index(self) -> usize {
    match self {
      Phase::A => 0,
      Phase::B => 1,
      Phase::C => 2,
      Phase::D => 3,
    }
  }
}

/// Absolute pixel position in the world.
///
/// Uses i64 for effectively infinite worlds without overflow concerns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WorldPos {
  pub x: i64,
  pub y: i64,
}

impl WorldPos {
  /// Creates a new world position.
  pub const fn new(x: i64, y: i64) -> Self {
    Self { x, y }
  }
}

/// Position in the chunk grid.
///
/// Each chunk spans [`CHUNK_SIZE`] pixels in each dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChunkPos {
  pub x: i32,
  pub y: i32,
}

impl ChunkPos {
  /// Creates a new chunk position.
  pub const fn new(x: i32, y: i32) -> Self {
    Self { x, y }
  }
}

/// Position within a chunk (0 to CHUNK_SIZE-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalPos {
  pub x: u16,
  pub y: u16,
}

impl LocalPos {
  /// Creates a new local position.
  pub const fn new(x: u16, y: u16) -> Self {
    Self { x, y }
  }
}

impl WorldPos {
  /// Convert to chunk position and local offset.
  ///
  /// Uses floor division for correct negative coordinate handling.
  /// For example, world position -1 maps to chunk -1 with local offset 511.
  pub fn to_chunk_and_local(self) -> (ChunkPos, LocalPos) {
    let chunk_size = CHUNK_SIZE as i64;

    // Floor division: for negative numbers, we need to round toward negative
    // infinity
    let cx = self.x.div_euclid(chunk_size) as i32;
    let cy = self.y.div_euclid(chunk_size) as i32;

    // Local offset is always positive (0 to CHUNK_SIZE-1)
    let lx = self.x.rem_euclid(chunk_size) as u16;
    let ly = self.y.rem_euclid(chunk_size) as u16;

    (ChunkPos::new(cx, cy), LocalPos::new(lx, ly))
  }
}

impl ChunkPos {
  /// Convert chunk origin to world position.
  ///
  /// Returns the bottom-left corner of the chunk in world coordinates.
  pub fn to_world(self) -> WorldPos {
    let chunk_size = CHUNK_SIZE as i64;
    WorldPos::new(self.x as i64 * chunk_size, self.y as i64 * chunk_size)
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TilePos {
  pub x: i64,
  pub y: i64,
}

impl TilePos {
  /// Creates a new tile position.
  pub const fn new(x: i64, y: i64) -> Self {
    Self { x, y }
  }
}

/// Dirty rectangle within a tile for simulation scheduling.
///
/// Coordinates are local to the tile (0 to TILE_SIZE-1).
/// Uses a two-phase cooldown: tiles stay active for 2 frames after
/// last activity to handle oscillating patterns in falling sand.
#[derive(Clone, Copy, Debug, Default)]
pub struct TileDirtyRect {
  /// Bounds for next frame (accumulated by expand() calls during simulation)
  next: Option<(u8, u8, u8, u8)>,
  /// Bounds to simulate this frame
  current: Option<(u8, u8, u8, u8)>,
  /// Frames until sleep (2 = active, 1 = cooling, 0 = sleeping)
  cooldown: u8,
}

impl TileDirtyRect {
  /// Creates an empty dirty rect (no pixels need simulation).
  pub const fn empty() -> Self {
    Self {
      next: None,
      current: None,
      cooldown: 0,
    }
  }

  /// Creates a dirty rect covering the entire tile.
  pub const fn full() -> Self {
    let full_bounds = Some((0, 0, (TILE_SIZE - 1) as u8, (TILE_SIZE - 1) as u8));
    Self {
      next: full_bounds,
      current: full_bounds,
      cooldown: 2,
    }
  }

  /// Returns true if no pixels need simulation this frame.
  pub fn is_empty(&self) -> bool {
    self.cooldown == 0
  }

  /// Expands the dirty rect to include the given local coordinate.
  /// Resets cooldown to 2 frames.
  pub fn expand(&mut self, x: u8, y: u8) {
    match &mut self.next {
      None => {
        self.next = Some((x, y, x, y));
      }
      Some((min_x, min_y, max_x, max_y)) => {
        *min_x = (*min_x).min(x);
        *min_y = (*min_y).min(y);
        *max_x = (*max_x).max(x);
        *max_y = (*max_y).max(y);
      }
    }
    self.cooldown = 2;
  }

  /// Advances to next frame: merges next into current, decrements cooldown.
  /// Call this at the start of tile simulation before bounds().
  pub fn tick(&mut self) {
    // Merge next into current (union of both rects)
    self.current = match (self.current, self.next) {
      (None, next) => next,
      (current, None) => current,
      (Some((c_min_x, c_min_y, c_max_x, c_max_y)), Some((n_min_x, n_min_y, n_max_x, n_max_y))) => {
        Some((
          c_min_x.min(n_min_x),
          c_min_y.min(n_min_y),
          c_max_x.max(n_max_x),
          c_max_y.max(n_max_y),
        ))
      }
    };

    // Decrement cooldown if no new activity
    if self.next.is_none() && self.cooldown > 0 {
      self.cooldown -= 1;
    }

    // Clear next for this frame's expand() calls
    self.next = None;

    // Sleep if cooldown expired
    if self.cooldown == 0 {
      self.current = None;
    }
  }

  /// Returns the bounds as (min_x, min_y, max_x, max_y), or None if sleeping.
  pub fn bounds(&self) -> Option<(u8, u8, u8, u8)> {
    if self.cooldown > 0 {
      self.current
    } else {
      None
    }
  }
}

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
    pos.x >= self.x
      && pos.x < self.x + self.width as i64
      && pos.y >= self.y
      && pos.y < self.y + self.height as i64
  }

  /// Returns the intersection of two rectangles, or None if they don't overlap.
  pub fn intersection(&self, other: &WorldRect) -> Option<WorldRect> {
    let x1 = self.x.max(other.x);
    let y1 = self.y.max(other.y);
    let x2 = (self.x + self.width as i64).min(other.x + other.width as i64);
    let y2 = (self.y + self.height as i64).min(other.y + other.height as i64);

    if x1 < x2 && y1 < y2 {
      Some(WorldRect {
        x: x1,
        y: y1,
        width: (x2 - x1) as u32,
        height: (y2 - y1) as u32,
      })
    } else {
      None
    }
  }

  /// Returns a new rectangle translated by the given offset.
  pub fn translate(&self, dx: i64, dy: i64) -> WorldRect {
    WorldRect {
      x: self.x + dx,
      y: self.y + dy,
      width: self.width,
      height: self.height,
    }
  }

  /// Returns the range of tile positions that overlap this rect.
  pub fn to_tile_range(&self) -> impl Iterator<Item = TilePos> {
    let tile_size = TILE_SIZE as i64;

    // Compute inclusive tile bounds using floor division
    let min_tx = self.x.div_euclid(tile_size);
    let min_ty = self.y.div_euclid(tile_size);
    let max_tx = (self.x + self.width as i64 - 1).div_euclid(tile_size);
    let max_ty = (self.y + self.height as i64 - 1).div_euclid(tile_size);

    (min_tx..=max_tx).flat_map(move |tx| (min_ty..=max_ty).map(move |ty| TilePos::new(tx, ty)))
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
