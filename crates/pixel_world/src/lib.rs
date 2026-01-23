//! Pixel World - Infinite cellular automata simulation plugin for Bevy.
//!
//! This crate provides a plugin for simulating infinite cellular automata
//! worlds.

use bevy::prelude::*;
use bevy::sprite_render::Material2dPlugin;

pub mod coords;
pub mod debug;
pub(crate) mod parallel;
pub mod material;
pub mod pixel;
pub mod primitives;
pub mod render;
pub mod seeding;
pub mod world;

pub use coords::{
    ChunkPos, ColorIndex, LocalPos, MaterialId, TilePos, WorldFragment, WorldPos, WorldRect,
    CHUNK_SIZE, TILE_SIZE,
};
pub use debug::{draw_text, rasterize_text, stamp_text, CpuFont, TextMask};
pub use material::{ids as material_ids, Material, Materials};
pub use pixel::{Pixel, PixelSurface};
pub use primitives::{Blitter, Chunk, LocalFragment, RgbaSurface, Surface};
pub use render::{
    create_chunk_quad, create_texture, materialize, spawn_static_chunk, upload_surface, ChunkMaterial,
    Rgba,
};
pub use seeding::{ChunkSeeder, MaterialSeeder, NoiseSeeder};
pub use world::{PixelWorld, PixelWorldBundle};
pub use world::plugin::{SharedChunkMesh, StreamingCamera};

pub use self::primitives::rect::Rect;

/// Plugin for infinite cellular automata simulation.
///
/// This plugin provides:
/// - Chunk material rendering
/// - Automatic chunk streaming based on camera position
/// - Async background seeding
/// - GPU texture upload for dirty chunks
///
/// To use automatic streaming, spawn a `PixelWorldBundle` and mark a camera
/// with `StreamingCamera`.
pub struct PixelWorldPlugin;

impl Plugin for PixelWorldPlugin {
  fn build(&self, app: &mut App) {
    // Embed the chunk shader
    bevy::asset::embedded_asset!(app, "render/shaders/chunk.wgsl");

    // Register the chunk material
    app.add_plugins(Material2dPlugin::<ChunkMaterial>::default());

    // Add world streaming systems
    app.add_plugins(world::plugin::PixelWorldStreamingPlugin);
  }
}
