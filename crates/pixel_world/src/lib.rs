//! Pixel World - Infinite cellular automata simulation plugin for Bevy.
//!
//! This crate provides a plugin for simulating infinite cellular automata worlds.

use bevy::prelude::*;

pub mod blitter;
pub mod chunk;
pub mod render;
pub mod surface;

pub use blitter::{Blitter, Rect};
pub use chunk::Chunk;
pub use render::{create_texture, upload_surface};
pub use surface::{Rgba, RgbaSurface, Surface};

/// Plugin for infinite cellular automata simulation.
pub struct PixelWorldPlugin;

impl Plugin for PixelWorldPlugin {
    fn build(&self, _app: &mut App) {
        // TODO: Implement cellular automata simulation
    }
}
