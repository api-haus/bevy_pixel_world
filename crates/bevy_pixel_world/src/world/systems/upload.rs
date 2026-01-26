//! GPU upload systems.
//!
//! Handles uploading dirty chunk data to GPU textures.

use bevy::prelude::*;

use super::super::{PixelWorld, SlotIndex};
use crate::render::{ChunkMaterial, upload_pixels};

/// Collects dirty, seeded slots that need GPU upload.
fn collect_dirty_slots(
  world: &PixelWorld,
) -> Vec<(SlotIndex, Handle<Image>, Handle<ChunkMaterial>)> {
  world
    .active_chunks()
    .filter_map(|(_, idx)| {
      let slot = world.slot(idx);
      if !slot.dirty || !slot.is_seeded() {
        return None;
      }
      Some((idx, slot.texture.clone()?, slot.material.clone()?))
    })
    .collect()
}

/// Uploads a slot's pixel data to its GPU texture.
fn upload_slot_to_gpu(
  world: &mut PixelWorld,
  idx: SlotIndex,
  texture_handle: &Handle<Image>,
  material_handle: &Handle<ChunkMaterial>,
  images: &mut Assets<Image>,
  materials: &mut Assets<ChunkMaterial>,
) {
  let slot = world.slot_mut(idx);

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
  #[cfg(feature = "diagnostics")] mut sim_metrics: ResMut<crate::diagnostics::SimulationMetrics>,
) {
  #[cfg(feature = "diagnostics")]
  let start = std::time::Instant::now();

  for mut world in worlds.iter_mut() {
    let dirty_slots = collect_dirty_slots(&world);

    for (idx, texture_handle, material_handle) in dirty_slots {
      upload_slot_to_gpu(
        &mut world,
        idx,
        &texture_handle,
        &material_handle,
        &mut images,
        &mut materials,
      );
    }
  }

  #[cfg(feature = "diagnostics")]
  {
    let elapsed_ms = start.elapsed().as_secs_f32() * 1000.0;
    sim_metrics.upload_time.push(elapsed_ms);
  }
}
