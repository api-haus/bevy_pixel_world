//! Pixel-perfect camera rendering.
//!
//! This module provides camera snapping and subpixel offset for artifact-free
//! pixel rendering. It eliminates gaps between chunks by rendering to a
//! low-resolution target where the camera is always pixel-aligned, then
//! blitting to the screen with a subpixel offset for smooth movement.
//!
//! # Usage
//!
//! ```ignore
//! use bevy_pixel_world::pixel_camera::{PixelCameraPlugin, PixelCamera, PixelCameraConfig};
//!
//! app.add_plugins(PixelCameraPlugin);
//!
//! // Mark your game camera with PixelCamera
//! commands.spawn((
//!     Camera2d,
//!     PixelCamera::default(),
//!     // ...
//! ));
//! ```
//!
//! # Architecture
//!
//! Uses two cameras:
//! 1. **Scene Camera**: Renders game content (layer 1) to low-res target
//! 2. **Blit Camera**: Renders blit quad (layer 0) to screen
//!
//! Chunks must be on `RenderLayers::layer(1)` to be rendered by the scene
//! camera.

mod components;
mod config;
mod material;
mod setup;
mod state;
mod systems;

use bevy::prelude::*;
use bevy::sprite_render::Material2dPlugin;
use bevy::transform::TransformSystems;
pub use components::{LogicalCameraPosition, PixelCamera};
pub use config::{PixelCameraConfig, PixelSizeMode};
pub use material::PixelBlitMaterial;
pub use setup::{
  FULLRES_SPRITE_LAYER, PixelBlitCamera, PixelBlitQuad, PixelFullresCamera, PixelSceneCamera,
};
pub use state::PixelCameraState;

/// System set for pixel camera systems.
///
/// Runs in `PostUpdate` after `TransformSystems::Propagate`.
/// Schedule camera follow systems to run **before** this set.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PixelCameraSet;

/// Plugin for pixel-perfect camera rendering.
///
/// Add this plugin after `DefaultPlugins` and before spawning cameras.
/// Mark your game camera with the `PixelCamera` component.
pub struct PixelCameraPlugin;

impl Plugin for PixelCameraPlugin {
  fn build(&self, app: &mut App) {
    // Only run if rendering is available
    if !app.is_plugin_added::<bevy::render::RenderPlugin>() {
      return;
    }

    // Embed the blit shader
    bevy::asset::embedded_asset!(app, "shaders/blit.wgsl");

    // Register material
    app.add_plugins(Material2dPlugin::<PixelBlitMaterial>::default());

    // Initialize resources
    app.init_resource::<PixelCameraConfig>();
    app.init_resource::<PixelCameraState>();

    // Configure PixelCameraSet to run after transform propagation
    app.configure_sets(
      PostUpdate,
      PixelCameraSet.after(TransformSystems::Propagate),
    );

    // Setup system runs in Update to wait for ortho area to be computed
    app.add_systems(
      Update,
      setup::setup_pixel_camera.run_if(not(pixel_camera_initialized)),
    );

    // Per-frame systems run in PostUpdate after transforms are propagated.
    // Camera follow systems should run BEFORE PixelCameraSet.
    app.add_systems(
      PostUpdate,
      (
        // Fix projections after Camera2d's required components override them
        setup::fix_camera_projections,
        systems::pixel_camera_store_logical,
        systems::pixel_camera_sync_fullres,
        systems::pixel_camera_snap,
        systems::pixel_camera_sync_state,
        systems::pixel_camera_handle_resize,
        systems::configure_egui_camera,
      )
        .chain()
        .in_set(PixelCameraSet)
        .run_if(pixel_camera_initialized),
    );
  }
}

/// Run condition: Returns true if the pixel camera is initialized.
fn pixel_camera_initialized(state: Res<PixelCameraState>) -> bool {
  state.initialized
}
