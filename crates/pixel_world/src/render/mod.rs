mod material;
mod render;

pub use material::ChunkMaterial;
pub use render::{create_chunk_quad, create_texture, spawn_static_chunk, upload_surface};
