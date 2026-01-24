//! Chunk - a spatial unit containing pixel data.
//!
//! A chunk is the basic unit of the world, containing a surface of pixels.
//!
//! See `docs/architecture/spatial-hierarchy.md` for the four-level spatial
//! organization. See `docs/architecture/chunk-pooling.md` for the pooling
//! lifecycle.

use crate::coords::{ChunkPos, TILES_PER_CHUNK, TILE_SIZE};
use crate::pixel::PixelSurface;

/// Number of tiles per chunk (16x16 = 256).
const TILE_COUNT: usize = (TILES_PER_CHUNK * TILES_PER_CHUNK) as usize;

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

/// A chunk of the world containing pixel data.
pub struct Chunk {
  /// Simulation data (material, color, damage, flags).
  pub pixels: PixelSurface,
  /// World position of this chunk. `None` when in the pool, `Some` when
  /// assigned.
  pos: Option<ChunkPos>,
  /// Per-tile dirty rectangles for simulation scheduling.
  tile_dirty_rects: Box<[TileDirtyRect]>,
}

impl Chunk {
  /// Creates a new chunk with the given dimensions.
  pub fn new(width: u32, height: u32) -> Self {
    Self {
      pixels: PixelSurface::new(width, height),
      pos: None,
      tile_dirty_rects: vec![TileDirtyRect::empty(); TILE_COUNT].into_boxed_slice(),
    }
  }

  /// Returns the world position of this chunk, if assigned.
  pub fn pos(&self) -> Option<ChunkPos> {
    self.pos
  }

  /// Sets the world position of this chunk.
  pub fn set_pos(&mut self, pos: ChunkPos) {
    self.pos = Some(pos);
  }

  /// Clears the world position (called when chunk returns to pool).
  pub fn clear_pos(&mut self) {
    self.pos = None;
  }

  /// Returns the dirty rect for the tile at (tx, ty) within this chunk.
  pub fn tile_dirty_rect(&self, tx: u32, ty: u32) -> &TileDirtyRect {
    let idx = (ty * TILES_PER_CHUNK + tx) as usize;
    &self.tile_dirty_rects[idx]
  }

  /// Returns a mutable reference to the dirty rect for the tile at (tx, ty).
  pub fn tile_dirty_rect_mut(&mut self, tx: u32, ty: u32) -> &mut TileDirtyRect {
    let idx = (ty * TILES_PER_CHUNK + tx) as usize;
    &mut self.tile_dirty_rects[idx]
  }

  /// Marks a pixel as dirty, expanding the appropriate tile's dirty rect.
  ///
  /// Also handles boundary propagation: if the pixel is at a tile edge,
  /// expands the adjacent tile's rect as well.
  pub fn mark_pixel_dirty(&mut self, local_x: u32, local_y: u32) {
    let tx = local_x / TILE_SIZE;
    let ty = local_y / TILE_SIZE;
    let px = (local_x % TILE_SIZE) as u8;
    let py = (local_y % TILE_SIZE) as u8;

    // Expand this tile's dirty rect
    self.tile_dirty_rect_mut(tx, ty).expand(px, py);

    // Boundary propagation within chunk
    let max_local = (TILE_SIZE - 1) as u8;

    // Left boundary: also expand tile to the left
    if px == 0 && tx > 0 {
      self.tile_dirty_rect_mut(tx - 1, ty).expand(max_local, py);
    }

    // Right boundary: also expand tile to the right
    if px == max_local && tx + 1 < TILES_PER_CHUNK {
      self.tile_dirty_rect_mut(tx + 1, ty).expand(0, py);
    }

    // Bottom boundary: also expand tile below
    if py == 0 && ty > 0 {
      self.tile_dirty_rect_mut(tx, ty - 1).expand(px, max_local);
    }

    // Top boundary: also expand tile above
    if py == max_local && ty + 1 < TILES_PER_CHUNK {
      self.tile_dirty_rect_mut(tx, ty + 1).expand(px, 0);
    }
  }

  /// Sets all tile dirty rects to full (entire tile needs simulation).
  pub fn set_all_dirty_rects_full(&mut self) {
    for rect in self.tile_dirty_rects.iter_mut() {
      *rect = TileDirtyRect::full();
    }
  }
}
