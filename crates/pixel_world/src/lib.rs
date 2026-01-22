//! Pixel World - Infinite cellular automata simulation plugin for Bevy.
//!
//! This crate provides a plugin for simulating infinite cellular automata
//! worlds.

use bevy::prelude::*;
use bevy::sprite_render::Material2dPlugin;

pub mod core;
pub mod debug;
pub mod render;

pub use core::{Blitter, Chunk, Rgba, RgbaSurface, Surface};

pub use debug::{draw_text, rasterize_text, stamp_text, CpuFont, TextMask};
pub use render::{
    create_chunk_quad, create_texture, spawn_static_chunk, upload_surface, ChunkMaterial,
};

pub use self::core::rect::Rect;

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
