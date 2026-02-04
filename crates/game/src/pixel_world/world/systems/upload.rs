//! GPU upload systems.
//!
//! Handles uploading dirty chunk data to GPU textures.

use bevy::prelude::*;
// WASM compat: std::time::Instant panics on wasm32
use web_time::Instant;

use super::super::{PixelWorld, SlotIndex};
use crate::pixel_world::diagnostics::profile;
use crate::pixel_world::render::{ChunkMaterial, upload_pixels};

/// Returns indices of dirty, seeded slots that need GPU upload.
fn dirty_slot_indices(world: &PixelWorld) -> impl Iterator<Item = SlotIndex> + '_ {
  world.active_chunks().filter_map(|(_, idx)| {
    let slot = world.slot(idx);
    (slot.dirty && slot.is_seeded() && slot.texture.is_some() && slot.material.is_some())
      .then_some(idx)
  })
}

/// Uploads a slot's pixel data to its GPU texture.
fn upload_slot_to_gpu(
  world: &mut PixelWorld,
  idx: SlotIndex,
  images: &mut Assets<Image>,
  materials: &mut Assets<ChunkMaterial>,
) {
  let slot = world.slot_mut(idx);

  // SAFETY: dirty_slot_indices() ensures these are Some
  let texture_handle = slot.texture.as_ref().unwrap();
  let material_handle = slot.material.as_ref().unwrap();

  if let Some(image) = images.get_mut(texture_handle) {
    upload_pixels(&slot.chunk.pixels, image);
  }

  // Touch material to force bind group refresh (Bevy workaround)
  let _ = materials.get_mut(material_handle);

  slot.dirty = false;
}

/// System: Uploads dirty chunks to GPU.
///
/// Uploads raw pixel data directly. Color lookup happens in the shader.
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
pub(crate) fn upload_dirty_chunks(
  mut worlds: Query<&mut PixelWorld>,
  mut images: ResMut<Assets<Image>>,
  mut materials: ResMut<Assets<ChunkMaterial>>,
  mut sim_metrics: ResMut<crate::pixel_world::diagnostics::SimulationMetrics>,
) {
  let _span = profile("upload_chunks");
  let start = Instant::now();

  for mut world in worlds.iter_mut() {
    // Collect indices first to avoid borrowing issues
    let dirty_indices: Vec<_> = dirty_slot_indices(&world).collect();

    for idx in dirty_indices {
      upload_slot_to_gpu(&mut world, idx, &mut images, &mut materials);
    }
  }

  let elapsed_ms = start.elapsed().as_secs_f32() * 1000.0;
  sim_metrics.upload_time.push(elapsed_ms);
}
