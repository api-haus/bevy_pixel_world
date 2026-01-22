//! Pixel World - Infinite cellular automata simulation plugin for Bevy.
//!
//! This crate provides a plugin for simulating infinite cellular automata
//! worlds.

use bevy::prelude::*;
use bevy::sprite_render::Material2dPlugin;

pub mod blitter;
pub mod chunk;
pub mod chunk_material;
pub mod font;
pub mod render;
pub mod surface;

pub use blitter::{Blitter, Rect};
pub use chunk::Chunk;
pub use chunk_material::ChunkMaterial;
pub use font::{draw_text, rasterize_text, stamp_text, CpuFont, TextMask};
pub use render::{create_chunk_quad, create_texture, spawn_static_chunk, upload_surface};
pub use surface::{Rgba, RgbaSurface, Surface};

/// Plugin for infinite cellular automata simulation.
pub struct PixelWorldPlugin;

impl Plugin for PixelWorldPlugin {
  fn build(&self, app: &mut App) {
    // Embed the chunk shader
    bevy::asset::embedded_asset!(app, "shaders/chunk.wgsl");

    // Register the chunk material
    app.add_plugins(Material2dPlugin::<ChunkMaterial>::default());
  }
}
