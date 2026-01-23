//! Chunk - a spatial unit containing pixel data.
//!
//! A chunk is the basic unit of the world, containing a surface of pixels.
//!
//! See `docs/architecture/spatial-hierarchy.md` for the four-level spatial
//! organization. See `docs/architecture/chunk-pooling.md` for the pooling
//! lifecycle.

use crate::coords::{ChunkPos, TileDirtyRect, TILE_SIZE, TILES_PER_CHUNK};
use crate::pixel::PixelSurface;

/// Number of tiles per chunk (16x16 = 256).
const TILE_COUNT: usize = (TILES_PER_CHUNK * TILES_PER_CHUNK) as usize;

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
