//! Chunk - a spatial unit containing pixel data.
//!
//! A chunk is the basic unit of the world, containing a surface of pixels.
//!
//! See `docs/architecture/spatial-hierarchy.md` for the four-level spatial
//! organization. See `docs/architecture/chunk-pooling.md` for the pooling
//! lifecycle.

use crate::pixel_world::coords::{CHUNK_SIZE, ChunkPos, TILE_SIZE, TILES_PER_CHUNK};
use crate::pixel_world::pixel::PixelSurface;

/// Pixels per heat cell edge.
pub const HEAT_CELL_SIZE: u32 = 4;
/// Number of heat cells per chunk edge.
pub const HEAT_GRID_SIZE: u32 = CHUNK_SIZE / HEAT_CELL_SIZE;
/// Total heat cells per chunk.
const HEAT_CELL_COUNT: usize = (HEAT_GRID_SIZE * HEAT_GRID_SIZE) as usize;

/// Number of tiles per chunk (16x16 = 256).
const TILE_COUNT: usize = (TILES_PER_CHUNK * TILES_PER_CHUNK) as usize;

/// Tile-local bounding box.
///
/// Coordinates are in the range 0..TILE_SIZE-1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileBounds {
  pub min_x: u8,
  pub min_y: u8,
  pub max_x: u8,
  pub max_y: u8,
}

/// Dirty rectangle within a tile for simulation scheduling.
///
/// Coordinates are local to the tile (0 to TILE_SIZE-1).
/// Uses a two-phase cooldown: tiles stay active for 2 frames after
/// last activity to handle oscillating patterns in falling sand.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct TileDirtyRect {
  /// Bounds for next frame (accumulated by expand() calls during simulation)
  next: Option<TileBounds>,
  /// Bounds to simulate this frame
  current: Option<TileBounds>,
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
    let full_bounds = Some(TileBounds {
      min_x: 0,
      min_y: 0,
      max_x: (TILE_SIZE - 1) as u8,
      max_y: (TILE_SIZE - 1) as u8,
    });
    Self {
      next: full_bounds,
      current: full_bounds,
      cooldown: 2,
    }
  }

  /// Expands the dirty rect to include the given local coordinate.
  /// Resets cooldown to 2 frames.
  pub fn expand(&mut self, x: u8, y: u8) {
    match &mut self.next {
      None => {
        self.next = Some(TileBounds {
          min_x: x,
          min_y: y,
          max_x: x,
          max_y: y,
        });
      }
      Some(bounds) => {
        bounds.min_x = bounds.min_x.min(x);
        bounds.min_y = bounds.min_y.min(y);
        bounds.max_x = bounds.max_x.max(x);
        bounds.max_y = bounds.max_y.max(y);
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
      (Some(c), Some(n)) => Some(TileBounds {
        min_x: c.min_x.min(n.min_x),
        min_y: c.min_y.min(n.min_y),
        max_x: c.max_x.max(n.max_x),
        max_y: c.max_y.max(n.max_y),
      }),
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

  /// Returns the bounds, or None if sleeping.
  pub fn bounds(&self) -> Option<TileBounds> {
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
  /// Per-tile collision dirty flags. When true, the tile's collision mesh
  /// needs regeneration.
  tile_collision_dirty: Box<[bool]>,
  /// True if this chunk was loaded from persistence (not procedurally
  /// generated).
  pub from_persistence: bool,
  /// Downsampled heat layer (128×128, ephemeral, not persisted).
  pub heat: Box<[u8]>,
}

impl Chunk {
  /// Creates a new chunk with the given dimensions.
  pub fn new(width: u32, height: u32) -> Self {
    Self {
      pixels: PixelSurface::new(width, height),
      pos: None,
      tile_dirty_rects: vec![TileDirtyRect::empty(); TILE_COUNT].into_boxed_slice(),
      tile_collision_dirty: vec![true; TILE_COUNT].into_boxed_slice(),
      from_persistence: false,
      heat: vec![0u8; HEAT_CELL_COUNT].into_boxed_slice(),
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
  pub(crate) fn tile_dirty_rect(&self, tx: u32, ty: u32) -> &TileDirtyRect {
    let idx = (ty * TILES_PER_CHUNK + tx) as usize;
    &self.tile_dirty_rects[idx]
  }

  /// Returns a mutable reference to the dirty rect for the tile at (tx, ty).
  pub(crate) fn tile_dirty_rect_mut(&mut self, tx: u32, ty: u32) -> &mut TileDirtyRect {
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

  /// Returns true if the tile at (tx, ty) has dirty collision geometry.
  pub fn is_tile_collision_dirty(&self, tx: u32, ty: u32) -> bool {
    let idx = (ty * TILES_PER_CHUNK + tx) as usize;
    self.tile_collision_dirty[idx]
  }

  /// Marks a tile's collision geometry as dirty.
  ///
  /// Also marks adjacent tiles at boundaries since collision meshes
  /// include a 1-pixel border.
  pub fn mark_tile_collision_dirty(&mut self, tx: u32, ty: u32) {
    let idx = (ty * TILES_PER_CHUNK + tx) as usize;
    self.tile_collision_dirty[idx] = true;
  }

  /// Marks a tile's collision geometry as clean.
  pub fn clear_tile_collision_dirty(&mut self, tx: u32, ty: u32) {
    let idx = (ty * TILES_PER_CHUNK + tx) as usize;
    self.tile_collision_dirty[idx] = false;
  }

  /// Sets all tile collision dirty flags to the given value.
  pub fn set_all_collision_dirty(&mut self, dirty: bool) {
    for flag in self.tile_collision_dirty.iter_mut() {
      *flag = dirty;
    }
  }

  /// Returns the heat value at heat cell (hx, hy).
  #[inline]
  pub fn heat_cell(&self, hx: u32, hy: u32) -> u8 {
    self.heat[(hy * HEAT_GRID_SIZE + hx) as usize]
  }

  /// Returns a mutable reference to the heat value at heat cell (hx, hy).
  #[inline]
  pub fn heat_cell_mut(&mut self, hx: u32, hy: u32) -> &mut u8 {
    &mut self.heat[(hy * HEAT_GRID_SIZE + hx) as usize]
  }

  /// Zeros all heat cells (called when chunk returns to pool).
  pub fn reset_heat(&mut self) {
    self.heat.fill(0);
  }

  /// Returns an iterator over (tx, ty) pairs for tiles with dirty collision.
  pub fn collision_dirty_tiles(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
    self
      .tile_collision_dirty
      .iter()
      .enumerate()
      .filter_map(|(idx, &dirty)| {
        if dirty {
          let tx = (idx as u32) % TILES_PER_CHUNK;
          let ty = (idx as u32) / TILES_PER_CHUNK;
          Some((tx, ty))
        } else {
          None
        }
      })
  }
}
