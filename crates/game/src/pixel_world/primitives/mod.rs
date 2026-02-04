mod chunk;
mod surface;

pub use chunk::{
  Chunk, HEAT_CELL_SIZE, HEAT_CELLS_PER_TILE, HEAT_GRID_SIZE, HEAT_TILES_PER_CHUNK,
  HeatDirtyTracker, TileBounds,
};
pub(crate) use surface::RgbaSurface;
pub use surface::Surface;
