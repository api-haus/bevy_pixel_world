//! Pixel World - Infinite cellular automata simulation plugin for Bevy.
//!
//! This crate provides a plugin for simulating infinite cellular automata
//! worlds.

use bevy::prelude::*;
use bevy::sprite_render::Material2dPlugin;

pub mod coords;
pub mod debug;
pub mod primitives;
pub mod render;
pub mod seeding;
pub mod streaming;

pub use coords::{ChunkPos, LocalPos, WorldPos, CHUNK_SIZE, POOL_SIZE, WINDOW_HEIGHT, WINDOW_WIDTH};
pub use primitives::{Blitter, Chunk, Fragment, Rgba, RgbaSurface, Surface};

pub use debug::{draw_text, rasterize_text, stamp_text, CpuFont, TextMask};
pub use render::{
  create_chunk_quad, create_texture, spawn_static_chunk, upload_surface, ChunkMaterial,
};
pub use seeding::{ChunkSeeder, NoiseSeeder};
pub use streaming::{ActiveChunk, ChunkPool, PoolHandle, StreamingWindow, WindowDelta};

pub use self::primitives::rect::Rect;

/// Plugin for infinite cellular automata simulation.
pub struct PixelWorldPlugin;

impl Plugin for PixelWorldPlugin {
  fn build(&self, app: &mut App) {
    // Embed the chunk shader
    bevy::asset::embedded_asset!(app, "render/shaders/chunk.wgsl");

    // Register the chunk material
    app.add_plugins(Material2dPlugin::<ChunkMaterial>::default());
  }
}
