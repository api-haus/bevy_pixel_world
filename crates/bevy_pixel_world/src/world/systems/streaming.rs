//! Chunk streaming systems.
//!
//! Handles streaming window updates and chunk entity lifecycle.

use bevy::prelude::*;

use super::super::persistence_systems::UnloadingChunks;
use super::super::{PixelWorld, SlotIndex};
use super::StreamingCamera;
use crate::coords::{CHUNK_SIZE, ChunkPos, WorldPos, WorldRect};
use crate::persistence::PersistenceTasks;
use crate::persistence::compression::compress_lz4;
use crate::persistence::format::StorageType;
#[cfg(not(feature = "headless"))]
use crate::render::{ChunkMaterial, create_pixel_texture};

/// System: Updates streaming windows based on camera position.
///
/// For each PixelWorld, checks if the camera has moved to a new chunk
/// and updates the streaming window accordingly.
#[allow(clippy::too_many_arguments)]
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
pub(crate) fn update_streaming_windows(
  mut commands: Commands,
  camera_query: Query<&GlobalTransform, With<StreamingCamera>>,
  mut worlds: Query<(Entity, &mut PixelWorld)>,
  #[cfg(not(feature = "headless"))] mut images: ResMut<Assets<Image>>,
  #[cfg(not(feature = "headless"))] mut materials: ResMut<Assets<ChunkMaterial>>,
  #[cfg(not(feature = "headless"))] palette: Option<Res<super::SharedPaletteTexture>>,
  mut persistence_tasks: ResMut<PersistenceTasks>,
  mut unloading_chunks: ResMut<UnloadingChunks>,
) {
  let Ok(camera_transform) = camera_query.single() else {
    return;
  };

  #[cfg(not(feature = "headless"))]
  let palette_handle = palette.as_ref().map(|p| p.handle.clone());

  // Convert camera position to chunk position
  // Offset by half chunk so transitions occur at chunk centers
  let half_chunk = (CHUNK_SIZE / 2) as i64;
  let cam_pos = camera_transform.translation();
  let cam_x = cam_pos.x as i64 + half_chunk;
  let cam_y = cam_pos.y as i64 + half_chunk;
  let (chunk_pos, _) = WorldPos::new(cam_x, cam_y).to_chunk_and_local();

  for (_world_entity, mut world) in worlds.iter_mut() {
    // Check if this is initial spawn (no active chunks yet)
    let needs_initial_spawn = world.active_count() == 0;

    let delta = if needs_initial_spawn {
      // Force initial spawn by setting center and getting all visible positions
      world.initialize_at(chunk_pos)
    } else {
      world.update_center(chunk_pos)
    };

    // Queue chunks that need saving
    for save_data in delta.to_save {
      // Compress full chunk data for storage
      let compressed = compress_lz4(&save_data.pixels);
      persistence_tasks.queue_save(save_data.pos, compressed, StorageType::Full);
    }

    // Despawn entities for chunks leaving the window
    for (pos, entity) in delta.to_despawn {
      unloading_chunks.positions.push(pos);
      commands.entity(entity).despawn();
    }

    // Spawn entities for chunks entering the window
    for (pos, slot_idx) in delta.to_spawn {
      spawn_chunk_entity(
        &mut commands,
        &mut world,
        #[cfg(not(feature = "headless"))]
        &mut images,
        #[cfg(not(feature = "headless"))]
        &mut materials,
        #[cfg(not(feature = "headless"))]
        palette_handle.clone(),
        pos,
        slot_idx,
      );
    }
  }
}

/// Spawns a chunk entity with transform and optional rendering components.
fn spawn_chunk_entity(
  commands: &mut Commands,
  world: &mut PixelWorld,
  #[cfg(not(feature = "headless"))] images: &mut Assets<Image>,
  #[cfg(not(feature = "headless"))] materials: &mut Assets<ChunkMaterial>,
  #[cfg(not(feature = "headless"))] palette_handle: Option<Handle<Image>>,
  pos: ChunkPos,
  slot_idx: SlotIndex,
) {
  // Spawn entity at chunk world position
  let world_pos = pos.to_world();
  let transform = Transform::from_xyz(
    world_pos.x as f32 + CHUNK_SIZE as f32 / 2.0,
    world_pos.y as f32 + CHUNK_SIZE as f32 / 2.0,
    0.0,
  );

  #[cfg(not(feature = "headless"))]
  let (entity, texture, material) = {
    let slot = world.slot_mut(slot_idx);

    // Create or reuse pixel texture (Rgba8Uint for raw pixel data)
    let texture = if let Some(tex) = slot.texture.take() {
      tex
    } else {
      create_pixel_texture(images, CHUNK_SIZE, CHUNK_SIZE)
    };

    // Create or reuse material
    let material = if let Some(mat) = slot.material.take() {
      mat
    } else {
      materials.add(ChunkMaterial {
        pixel_texture: Some(texture.clone()),
        palette_texture: palette_handle.clone(),
      })
    };

    // Update material textures if reusing
    if let Some(mat) = materials.get_mut(&material) {
      mat.pixel_texture = Some(texture.clone());
      mat.palette_texture = palette_handle;
    }

    let mesh = world.mesh().clone();
    let entity = commands
      .spawn((
        Mesh2d(mesh),
        transform,
        Visibility::default(),
        MeshMaterial2d(material.clone()),
      ))
      .id();

    (entity, texture, material)
  };

  #[cfg(feature = "headless")]
  let entity = commands.spawn(transform).id();

  world.register_slot_entity(
    slot_idx,
    entity,
    #[cfg(not(feature = "headless"))]
    texture,
    #[cfg(not(feature = "headless"))]
    material,
  );
}

/// System: Updates simulation bounds from camera viewport.
///
/// Extracts the visible area from the streaming camera's orthographic
/// projection and sets it as the simulation bounds for all pixel worlds.
pub(crate) fn update_simulation_bounds(
  camera_query: Query<(&GlobalTransform, &Projection), With<StreamingCamera>>,
  mut worlds: Query<&mut PixelWorld>,
) {
  let Ok((transform, projection)) = camera_query.single() else {
    return;
  };

  // Extract orthographic projection, skip if perspective
  let Projection::Orthographic(ortho) = projection else {
    return;
  };

  let cam_pos = transform.translation();

  // Extract viewport dimensions from the orthographic projection area
  let half_width = (ortho.area.max.x - ortho.area.min.x) / 2.0;
  let half_height = (ortho.area.max.y - ortho.area.min.y) / 2.0;

  // Skip if area is not yet initialized (Bevy computes it after first frame)
  if half_width <= 0.0 || half_height <= 0.0 {
    return;
  }

  let bounds = WorldRect::new(
    (cam_pos.x - half_width) as i64,
    (cam_pos.y - half_height) as i64,
    (half_width * 2.0) as u32,
    (half_height * 2.0) as u32,
  );

  for mut world in worlds.iter_mut() {
    world.set_simulation_bounds(Some(bounds));
  }
}
