//! Pixel World - Infinite cellular automata simulation plugin for Bevy.
//!
//! This crate provides a plugin for simulating infinite cellular automata
//! worlds.

use bevy::prelude::*;
use bevy::sprite_render::Material2dPlugin;

pub mod coords;
pub mod debug;
pub mod debug_shim;
pub(crate) mod parallel;
pub mod material;
pub mod pixel;
pub mod primitives;
pub mod render;
pub mod seeding;
pub mod simulation;
#[cfg(feature = "visual-debug")]
pub mod visual_debug;
pub mod world;

pub use coords::{
    ChunkPos, ColorIndex, LocalPos, MaterialId, TilePos, WorldFragment, WorldPos, WorldRect,
    CHUNK_SIZE, TILE_SIZE,
};
pub use debug::{draw_text, rasterize_text, stamp_text, CpuFont, TextMask};
pub use material::{ids as material_ids, Material, Materials, PhysicsState};
pub use pixel::{Pixel, PixelSurface};
pub use primitives::{Chunk, RgbaSurface, Surface};
pub use render::{
    create_chunk_quad, create_palette_texture, create_pixel_texture, create_texture, materialize,
    spawn_static_chunk, upload_palette, upload_pixels, upload_surface, ChunkMaterial, Rgba,
};
pub use seeding::{ChunkSeeder, MaterialSeeder, NoiseSeeder};
pub use simulation::simulate_tick;
pub use world::{PixelWorld, PixelWorldBundle, SpawnPixelWorld};
pub use world::plugin::{SharedChunkMesh, SharedPaletteTexture, StreamingCamera};

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

    // Initialize Materials registry (users can override by inserting before plugin)
    app.init_resource::<Materials>();

    // Add world streaming systems
    app.add_plugins(world::plugin::PixelWorldStreamingPlugin);

    // Add visual debug plugin if feature is enabled
    #[cfg(feature = "visual-debug")]
    app.add_plugins(visual_debug::VisualDebugPlugin);
  }
}
