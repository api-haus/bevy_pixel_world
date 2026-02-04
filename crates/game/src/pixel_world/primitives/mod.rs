mod chunk;
mod surface;

pub use chunk::{Chunk, HEAT_CELL_SIZE, HEAT_GRID_SIZE, TileBounds};
pub(crate) use surface::RgbaSurface;
pub use surface::Surface;
