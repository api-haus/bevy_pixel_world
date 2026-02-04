//! Streaming window update systems.
//!
//! Handles camera-based streaming window updates and simulation bounds.

use bevy::prelude::*;

use super::UnloadingChunks;
use crate::pixel_world::coords::{CHUNK_SIZE, ChunkPos, WorldPos, WorldRect};
use crate::pixel_world::persistence::PersistenceTasks;
use crate::pixel_world::persistence::compression::compress_lz4;
use crate::pixel_world::persistence::format::StorageType;
use crate::pixel_world::pixel_camera::LogicalCameraPosition;
use crate::pixel_world::primitives::HEAT_GRID_SIZE;
use crate::pixel_world::render::{ChunkMaterial, create_heat_texture, create_pixel_texture};
use crate::pixel_world::world::control::{PendingPersistenceInit, PersistenceControl};
use crate::pixel_world::world::slot::ChunkLifecycle;
use crate::pixel_world::world::{PixelWorld, SlotIndex};

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

/// System: Updates streaming windows based on camera position.
///
/// For each PixelWorld, checks if the camera has moved to a new chunk
/// and updates the streaming window accordingly.
#[allow(clippy::too_many_arguments)]
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
pub(crate) fn update_streaming_windows(
  mut commands: Commands,
  camera_query: Query<(&GlobalTransform, Option<&LogicalCameraPosition>), With<StreamingCamera>>,
  mut worlds: Query<(Entity, &mut PixelWorld)>,
  mut images: Option<ResMut<Assets<Image>>>,
  mut materials: Option<ResMut<Assets<ChunkMaterial>>>,
  palette: Option<Res<SharedPaletteTexture>>,
  mut persistence_tasks: ResMut<PersistenceTasks>,
  mut unloading_chunks: ResMut<UnloadingChunks>,
  persistence_control: Option<Res<PersistenceControl>>,
  pending_init: Option<Res<PendingPersistenceInit>>,
) {
  let Ok((camera_transform, logical_pos)) = camera_query.single() else {
    return;
  };

  let palette_handle = palette.as_ref().map(|p| p.handle.clone());
  // Check if persistence is available AND enabled (not in editor mode).
  // Also check pending init for WASM async initialization.
  let persistence_enabled =
    persistence_control.as_ref().is_some_and(|p| p.is_enabled()) || pending_init.is_some();

  // Use logical camera position if available (pixel camera mode)
  // Otherwise fall back to transform position
  let cam_pos = logical_pos
    .map(|lp| Vec3::new(lp.0.x, lp.0.y, 0.0))
    .unwrap_or_else(|| camera_transform.translation());

  // Convert camera position to chunk position
  // Offset by half chunk so transitions occur at chunk centers
  let half_chunk = (CHUNK_SIZE / 2) as i64;
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
      // When persistence is enabled, start in Loading state to check for saved data.
      // When disabled (editor mode), skip Loading and go straight to Seeding.
      if persistence_enabled {
        let slot = world.slot_mut(slot_idx);
        slot.lifecycle = ChunkLifecycle::Loading;
      }

      spawn_chunk_entity(
        &mut commands,
        &mut world,
        images.as_deref_mut(),
        materials.as_deref_mut(),
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
  images: Option<&mut Assets<Image>>,
  materials: Option<&mut Assets<ChunkMaterial>>,
  palette_handle: Option<Handle<Image>>,
  pos: ChunkPos,
  slot_idx: SlotIndex,
) {
  // Spawn entity at chunk's min corner (mesh origin is bottom-left)
  let world_pos = pos.to_world();
  let transform = Transform::from_xyz(world_pos.x as f32, world_pos.y as f32, 0.0);

  let (entity, texture, material, heat_tex) =
    if let (Some(images), Some(materials)) = (images, materials) {
      let slot = world.slot_mut(slot_idx);

      // Create or reuse pixel texture (Rgba8Uint for raw pixel data)
      let texture = if let Some(tex) = slot.texture.take() {
        tex
      } else {
        create_pixel_texture(images, CHUNK_SIZE, CHUNK_SIZE)
      };

      // Create or reuse heat texture (R8Unorm for bilinear sampling)
      let heat_tex = if let Some(tex) = slot.heat_texture.take() {
        tex
      } else {
        create_heat_texture(images, HEAT_GRID_SIZE, HEAT_GRID_SIZE)
      };

      // Create or reuse material
      let material = if let Some(mat) = slot.material.take() {
        mat
      } else {
        materials.add(ChunkMaterial {
          pixel_texture: Some(texture.clone()),
          palette_texture: palette_handle.clone(),
          heat_texture: Some(heat_tex.clone()),
        })
      };

      // Update material textures if reusing
      if let Some(mat) = materials.get_mut(&material) {
        mat.pixel_texture = Some(texture.clone());
        mat.palette_texture = palette_handle;
        mat.heat_texture = Some(heat_tex.clone());
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

      (entity, Some(texture), Some(material), Some(heat_tex))
    } else {
      let entity = commands.spawn(transform).id();
      (entity, None, None, None)
    };

  world.register_slot_entity(slot_idx, entity, texture, material, heat_tex);
}

/// System: Updates simulation bounds from camera viewport.
///
/// Extracts the visible area from the streaming camera's orthographic
/// projection and sets it as the simulation bounds for all pixel worlds.
pub(crate) fn update_simulation_bounds(
  camera_query: Query<
    (
      &GlobalTransform,
      &Projection,
      Option<&LogicalCameraPosition>,
    ),
    With<StreamingCamera>,
  >,
  mut worlds: Query<&mut PixelWorld>,
) {
  let Ok((transform, projection, logical_pos)) = camera_query.single() else {
    return;
  };

  // Extract orthographic projection, skip if perspective
  let Projection::Orthographic(ortho) = projection else {
    return;
  };

  // Use logical camera position if available (pixel camera mode)
  // Otherwise fall back to transform position
  let cam_pos = logical_pos
    .map(|lp| Vec3::new(lp.0.x, lp.0.y, 0.0))
    .unwrap_or_else(|| transform.translation());

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
