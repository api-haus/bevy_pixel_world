//! PixelWorld ECS systems organized by responsibility.

mod seeding;
mod streaming;
#[cfg(not(feature = "headless"))]
mod upload;

use bevy::prelude::*;
#[cfg(not(feature = "headless"))]
pub(crate) use seeding::poll_seeding_tasks;
pub(crate) use seeding::{SeedingTasks, dispatch_seeding};
pub(crate) use streaming::{update_simulation_bounds, update_streaming_windows};
#[cfg(not(feature = "headless"))]
pub(crate) use upload::upload_dirty_chunks;

/// Marker component for the main camera that controls streaming.
#[derive(Component)]
pub struct StreamingCamera;

/// Shared mesh resource for chunk quads.
#[derive(Resource)]
pub(crate) struct SharedChunkMesh(pub Handle<Mesh>);

/// Shared palette texture for GPU-side color lookup.
#[derive(Resource)]
pub(crate) struct SharedPaletteTexture {
  pub handle: Handle<Image>,
  /// Whether the palette has been populated from Materials.
  pub initialized: bool,
}
