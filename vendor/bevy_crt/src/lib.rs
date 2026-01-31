//! CRT post-processing effect for Bevy 0.17.
//!
//! Provides realistic CRT monitor simulation based on crt-guest-advanced-hd
//! shaders.
//!
//! # Features
//!
//! - Phosphor afterglow/persistence
//! - Color temperature and gamma adjustments
//! - Horizontal and vertical filtering/sharpening
//! - Bloom and glow effects
//! - Scanline simulation
//! - CRT shadow mask patterns
//! - Screen curvature
//! - Deconvergence (RGB channel separation)
//!
//! # Usage with PixelCamera (bevy_pixel_world)
//!
//! When used with bevy_pixel_world's PixelCamera, the CRT effect is
//! automatically inserted after the pixel-perfect blit pass:
//!
//! ```ignore
//! use bevy::prelude::*;
//! use bevy_pixel_world::PixelWorldPlugin;
//! use bevy_crt::Crt2dPlugin;
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugins(PixelWorldPlugin)
//!         .add_plugins(Crt2dPlugin)  // Automatically integrates with PixelCamera
//!         .run();
//! }
//! ```
//!
//! # Standalone Usage
//!
//! Without PixelCamera, mark your camera with `CrtSourceCamera`:
//!
//! ```ignore
//! use bevy::prelude::*;
//! use bevy_crt::{Crt2dPlugin, CrtSourceCamera};
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugins(Crt2dPlugin)
//!         .add_systems(Startup, setup)
//!         .run();
//! }
//!
//! fn setup(mut commands: Commands) {
//!     commands.spawn((
//!         Camera2d,
//!         CrtSourceCamera,
//!     ));
//! }
//! ```
//!
//! # License
//!
//! GPL-3.0-or-later (inherited from original crt-guest-advanced-hd shaders)

pub mod integration;
mod materials;
pub mod plugin;

pub use materials::*;
pub use plugin::{Crt2dPlugin, CrtConfig, CrtSourceCamera, CrtState, spawn_crt_pass};
