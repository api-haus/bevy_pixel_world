//! ECS plugin and systems for PixelWorld.
//!
//! Provides automatic chunk streaming, seeding, and GPU upload.

use std::collections::HashSet;

use bevy::prelude::*;
#[cfg(not(feature = "headless"))]
use bevy::tasks::{AsyncComputeTaskPool, Task};

use super::control::{
  PersistenceComplete, PersistenceControl, RequestPersistence, SimulationState,
};
pub use super::persistence::{LoadedChunks, UnloadingChunks};
use super::persistence::{
  clear_chunk_tracking, flush_persistence_queue, handle_persistence_messages,
  notify_persistence_complete, process_pending_save_requests, save_pixel_bodies_on_chunk_unload,
  save_pixel_bodies_on_request, tick_auto_save_timer,
};
use super::{PixelWorld, SlotIndex};
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
use crate::collision::physics::{PhysicsColliderRegistry, sync_physics_colliders};
use crate::collision::{
  CollisionCache, CollisionConfig, CollisionTasks, dispatch_collision_tasks,
  invalidate_dirty_tiles, poll_collision_tasks,
};
#[cfg(feature = "visual_debug")]
use crate::collision::{
  SampleMesh, draw_collision_gizmos, draw_sample_mesh_gizmos, update_sample_mesh,
};
use crate::coords::{CHUNK_SIZE, ChunkPos, TilePos, WorldPos, WorldRect};
use crate::culling::{CullingConfig, update_entity_culling};
use crate::debug_shim;
use crate::material::Materials;
use crate::persistence::{
  PersistenceTasks, PixelBodyRecord, WorldSaveResource, compression::compress_lz4,
  format::StorageType,
};
use crate::pixel_body::{
  DisplacementState, LastBlitTransform, Persistable, PixelBodyId, PixelBodyIdGenerator,
  apply_readback_changes, detect_external_erasure, readback_pixel_bodies, split_pixel_bodies,
  update_pixel_bodies,
};
use crate::primitives::Chunk;
#[cfg(not(feature = "headless"))]
use crate::render::{
  ChunkMaterial, create_chunk_quad, create_palette_texture, create_pixel_texture, upload_palette,
  upload_pixels,
};
use crate::simulation;

/// Marker component for the main camera that controls streaming.
#[derive(Component)]
pub struct StreamingCamera;

/// Resource holding async seeding tasks.
#[derive(Resource, Default)]
pub(crate) struct SeedingTasks {
  #[cfg(not(feature = "headless"))]
  tasks: Vec<SeedingTask>,
}

/// An in-flight seeding task.
#[cfg(not(feature = "headless"))]
struct SeedingTask {
  /// Which PixelWorld entity.
  world_entity: Entity,
  /// Which slot is being seeded.
  slot_index: SlotIndex,
  /// The chunk position being seeded.
  pos: ChunkPos,
  /// The async task returning a seeded chunk.
  task: Task<Chunk>,
}

/// Maximum number of concurrent seeding tasks.
#[cfg(not(feature = "headless"))]
const MAX_SEEDING_TASKS: usize = 2;

/// Creates and seeds a new chunk at the given position.
fn seed_chunk(seeder: &(dyn crate::seeding::ChunkSeeder + Send + Sync), pos: ChunkPos) -> Chunk {
  let mut chunk = Chunk::new(CHUNK_SIZE, CHUNK_SIZE);
  chunk.set_pos(pos);
  seeder.seed(pos, &mut chunk);
  chunk
}

/// Merges seeded pixels into existing chunk, preserving PIXEL_BODY pixels.
///
/// When seeding completes asynchronously, pixel bodies may have already
/// blitted to the chunk. We must not overwrite those pixels or they'll
/// be detected as destroyed.
fn merge_seeded_pixels(
  existing: &mut crate::pixel::PixelSurface,
  seeded: &crate::pixel::PixelSurface,
) {
  use crate::pixel::PixelFlags;

  let existing_slice = existing.as_slice_mut();
  let seeded_slice = seeded.as_slice();

  for (existing_pixel, seeded_pixel) in existing_slice.iter_mut().zip(seeded_slice.iter()) {
    // Only overwrite if existing pixel doesn't have PIXEL_BODY flag
    if !existing_pixel.flags.contains(PixelFlags::PIXEL_BODY) {
      *existing_pixel = *seeded_pixel;
    }
  }
}

/// Entry for a body waiting to spawn.
struct PendingBodyEntry {
  record: PixelBodyRecord,
  required_tiles: Vec<TilePos>,
}

/// Bodies waiting for collision tiles before spawning.
#[derive(Resource, Default)]
pub struct PendingPixelBodies {
  entries: Vec<PendingBodyEntry>,
}

/// Internal plugin for PixelWorld streaming systems.
///
/// This is automatically added by the main `PixelWorldPlugin`.
/// Do not add this plugin directly.
pub(crate) struct PixelWorldStreamingPlugin;

impl Plugin for PixelWorldStreamingPlugin {
  fn build(&self, app: &mut App) {
    app
      .init_resource::<SeedingTasks>()
      .init_resource::<PersistenceTasks>()
      .init_resource::<CollisionCache>()
      .init_resource::<CollisionTasks>()
      .init_resource::<CollisionConfig>()
      .init_resource::<CullingConfig>()
      .init_resource::<UnloadingChunks>()
      .init_resource::<LoadedChunks>()
      .init_resource::<PendingPixelBodies>()
      .init_resource::<PixelBodyIdGenerator>()
      .init_resource::<SimulationState>()
      .init_resource::<PersistenceControl>()
      .add_message::<RequestPersistence>()
      .add_message::<PersistenceComplete>();

    #[cfg(not(feature = "headless"))]
    app.add_systems(PreStartup, setup_shared_resources);

    #[cfg(feature = "visual_debug")]
    app.init_resource::<SampleMesh>();

    #[cfg(any(feature = "avian2d", feature = "rapier2d"))]
    {
      app.init_resource::<PhysicsColliderRegistry>();
      app.add_systems(Update, sync_physics_colliders.after(poll_collision_tasks));
    }

    #[cfg(not(feature = "headless"))]
    app.add_systems(
      Update,
      (
        // Pre-simulation group
        (
          clear_chunk_tracking,
          tick_auto_save_timer,
          handle_persistence_messages,
          initialize_palette,
          tick_pixel_worlds,
          save_pixel_bodies_on_chunk_unload,
          update_entity_culling,
          dispatch_seeding,
          poll_seeding_tasks,
          queue_pixel_bodies_on_chunk_load,
          update_simulation_bounds,
        )
          .chain(),
        // Simulation group
        (
          detect_external_erasure,
          update_pixel_bodies,
          run_simulation.run_if(simulation_not_paused),
          readback_pixel_bodies,
          apply_readback_changes,
          split_pixel_bodies,
          invalidate_dirty_tiles,
        )
          .chain(),
        // Post-simulation group
        (
          dispatch_collision_tasks,
          poll_collision_tasks,
          spawn_pending_pixel_bodies,
          upload_dirty_chunks,
          process_pending_save_requests,
          save_pixel_bodies_on_request,
          flush_persistence_queue,
          notify_persistence_complete,
        )
          .chain(),
      )
        .chain(),
    );

    #[cfg(all(not(feature = "headless"), feature = "visual_debug"))]
    app.add_systems(
      PostUpdate,
      (
        update_sample_mesh,
        draw_collision_gizmos,
        draw_sample_mesh_gizmos,
      ),
    );

    #[cfg(feature = "headless")]
    app.add_systems(
      Update,
      (
        // Pre-simulation group
        (
          clear_chunk_tracking,
          tick_auto_save_timer,
          handle_persistence_messages,
          tick_pixel_worlds,
          save_pixel_bodies_on_chunk_unload,
          update_entity_culling,
          dispatch_seeding,
          queue_pixel_bodies_on_chunk_load,
          update_simulation_bounds,
        )
          .chain(),
        // Simulation group
        (
          detect_external_erasure,
          update_pixel_bodies,
          run_simulation.run_if(simulation_not_paused),
          readback_pixel_bodies,
          apply_readback_changes,
          split_pixel_bodies,
          invalidate_dirty_tiles,
        )
          .chain(),
        // Post-simulation group
        (
          dispatch_collision_tasks,
          poll_collision_tasks,
          spawn_pending_pixel_bodies,
          process_pending_save_requests,
          save_pixel_bodies_on_request,
          flush_persistence_queue,
          notify_persistence_complete,
        )
          .chain(),
      )
        .chain(),
    );
  }
}

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

/// Sets up shared resources used by all PixelWorlds.
///
/// Runs in PreStartup to ensure resources are available before user Startup
/// systems that spawn PixelWorlds.
#[cfg(not(feature = "headless"))]
fn setup_shared_resources(world: &mut World) {
  let mesh = {
    let mut meshes = world.resource_mut::<Assets<Mesh>>();
    meshes.add(create_chunk_quad(CHUNK_SIZE as f32, CHUNK_SIZE as f32))
  };
  world.insert_resource(SharedChunkMesh(mesh));

  let palette = {
    let mut images = world.resource_mut::<Assets<Image>>();
    create_palette_texture(&mut images)
  };
  world.insert_resource(SharedPaletteTexture {
    handle: palette,
    initialized: false,
  });
}

/// System: Initializes the palette texture when Materials becomes available.
#[cfg(not(feature = "headless"))]
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
fn initialize_palette(
  mut palette: ResMut<SharedPaletteTexture>,
  mut images: ResMut<Assets<Image>>,
  mat_registry: Option<Res<Materials>>,
) {
  if palette.initialized {
    return;
  }

  let Some(mat_registry) = mat_registry else {
    return;
  };

  if let Some(image) = images.get_mut(&palette.handle) {
    upload_palette(&mat_registry, image);
    palette.initialized = true;
  }
}

/// System: Updates streaming windows based on camera position.
///
/// For each PixelWorld, checks if the camera has moved to a new chunk
/// and updates the streaming window accordingly.
#[allow(clippy::too_many_arguments)]
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
fn tick_pixel_worlds(
  mut commands: Commands,
  camera_query: Query<&GlobalTransform, With<StreamingCamera>>,
  mut worlds: Query<(Entity, &mut PixelWorld)>,
  #[cfg(not(feature = "headless"))] mut images: ResMut<Assets<Image>>,
  #[cfg(not(feature = "headless"))] mut materials: ResMut<Assets<ChunkMaterial>>,
  #[cfg(not(feature = "headless"))] palette: Option<Res<SharedPaletteTexture>>,
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

/// Collects in-flight seeding task count and slot indices for a world entity.
#[cfg(not(feature = "headless"))]
fn collect_in_flight_tasks(
  tasks: &[SeedingTask],
  world_entity: Entity,
) -> (usize, HashSet<SlotIndex>) {
  let mut count = 0;
  let mut slots = HashSet::new();
  for task in tasks {
    if task.world_entity == world_entity {
      count += 1;
      slots.insert(task.slot_index);
    }
  }
  (count, slots)
}

/// Spawns an async seeding task for a chunk.
#[cfg(not(feature = "headless"))]
fn spawn_seeding_task(
  seeding_tasks: &mut SeedingTasks,
  task_pool: &bevy::tasks::AsyncComputeTaskPool,
  world_entity: Entity,
  world: &PixelWorld,
  pos: crate::coords::ChunkPos,
  slot_idx: SlotIndex,
) {
  let seeder = world.seeder().clone();
  let task = task_pool.spawn(async move { seed_chunk(seeder.as_ref(), pos) });

  seeding_tasks.tasks.push(SeedingTask {
    world_entity,
    slot_index: slot_idx,
    pos,
    task,
  });
}

/// System: Dispatches async seeding tasks for unseeded chunks.
#[cfg(not(feature = "headless"))]
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
fn dispatch_seeding(
  mut seeding_tasks: ResMut<SeedingTasks>,
  mut worlds: Query<(Entity, &mut PixelWorld)>,
) {
  let task_pool = AsyncComputeTaskPool::get();

  for (world_entity, world) in worlds.iter_mut() {
    let (mut in_flight, in_flight_slots) =
      collect_in_flight_tasks(&seeding_tasks.tasks, world_entity);

    if in_flight >= MAX_SEEDING_TASKS {
      continue;
    }

    for (pos, slot_idx) in world.active_chunks() {
      if in_flight_slots.contains(&slot_idx) {
        continue;
      }

      let slot = world.slot(slot_idx);
      if slot.is_seeded() {
        continue;
      }

      spawn_seeding_task(
        &mut seeding_tasks,
        task_pool,
        world_entity,
        &world,
        pos,
        slot_idx,
      );

      in_flight += 1;
      if in_flight >= MAX_SEEDING_TASKS {
        break;
      }
    }
  }
}

/// System: Seeds chunks synchronously in headless mode.
///
/// In headless mode, we seed synchronously instead of using async tasks
/// because the async task pool may not work reliably in test environments.
#[cfg(feature = "headless")]
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
fn dispatch_seeding(
  mut worlds: Query<&mut PixelWorld>,
  mut loaded_chunks: ResMut<LoadedChunks>,
  gizmos: debug_shim::GizmosParam,
) {
  let debug_gizmos = gizmos.get();

  for mut world in worlds.iter_mut() {
    // Collect unseeded chunks
    let unseeded: Vec<_> = world
      .active_chunks()
      .filter(|(_, idx)| !world.slot(*idx).is_seeded())
      .collect();

    for (pos, slot_idx) in unseeded {
      // Seed synchronously
      let seeded_chunk = seed_chunk(world.seeder().as_ref(), pos);

      // Merge seeded data into slot, preserving PIXEL_BODY pixels
      let slot = world.slot_mut(slot_idx);
      merge_seeded_pixels(&mut slot.chunk.pixels, &seeded_chunk.pixels);
      slot.chunk.set_all_dirty_rects_full();
      slot.lifecycle = super::slot::ChunkLifecycle::Active;
      slot.dirty = true;

      // If loaded from disk, mark as persisted
      if seeded_chunk.from_persistence {
        slot.persisted = true;
      }

      // Track that this chunk just loaded
      loaded_chunks.positions.push(pos);

      debug_shim::emit_chunk(debug_gizmos, pos);
    }
  }
}

/// System: Polls completed seeding tasks and swaps in seeded chunks.
#[cfg(not(feature = "headless"))]
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
fn poll_seeding_tasks(
  mut seeding_tasks: ResMut<SeedingTasks>,
  mut worlds: Query<&mut PixelWorld>,
  mut loaded_chunks: ResMut<LoadedChunks>,
  gizmos: debug_shim::GizmosParam,
) {
  let debug_gizmos = gizmos.get();

  seeding_tasks.tasks.retain_mut(|task| {
    if !task.task.is_finished() {
      return true; // keep pending tasks
    }

    let seeded_chunk = bevy::tasks::block_on(&mut task.task);

    if let Ok(mut world) = worlds.get_mut(task.world_entity)
    // Slot may have been recycled if camera moved while task was in flight.
    // Both checks are needed: position mapping and slot index must match.
    && let Some(current_idx) = world.get_slot_index(task.pos)
    && current_idx == task.slot_index
    {
      let slot = world.slot_mut(task.slot_index);
      // Merge seeded pixels, preserving any PIXEL_BODY pixels that were
      // blitted before seeding completed.
      merge_seeded_pixels(&mut slot.chunk.pixels, &seeded_chunk.pixels);
      slot.chunk.set_all_dirty_rects_full();
      slot.lifecycle = super::slot::ChunkLifecycle::Active;
      slot.dirty = true;

      // If loaded from disk, mark as persisted (no need to save again)
      if seeded_chunk.from_persistence {
        slot.persisted = true;
      }

      // Track that this chunk just loaded
      loaded_chunks.positions.push(task.pos);

      debug_shim::emit_chunk(debug_gizmos, task.pos);
    }

    false // remove completed task
  });
}

/// System: Updates simulation bounds from camera viewport.
///
/// Extracts the visible area from the streaming camera's orthographic
/// projection and sets it as the simulation bounds for all pixel worlds.
fn update_simulation_bounds(
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

/// System: Runs cellular automata simulation on all pixel worlds.
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
fn run_simulation(
  mut worlds: Query<&mut PixelWorld>,
  mat_registry: Option<Res<Materials>>,
  gizmos: debug_shim::GizmosParam,
  #[cfg(feature = "diagnostics")] mut sim_metrics: ResMut<crate::diagnostics::SimulationMetrics>,
) {
  let Some(materials) = mat_registry else {
    return;
  };

  let debug_gizmos = gizmos.get();

  #[cfg(feature = "diagnostics")]
  let start = std::time::Instant::now();

  for mut world in worlds.iter_mut() {
    simulation::simulate_tick(&mut world, &materials, debug_gizmos);
  }

  #[cfg(feature = "diagnostics")]
  {
    let elapsed_ms = start.elapsed().as_secs_f32() * 1000.0;
    sim_metrics.sim_time.push(elapsed_ms);
  }
}

/// Collects dirty, seeded slots that need GPU upload.
#[cfg(not(feature = "headless"))]
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
#[cfg(not(feature = "headless"))]
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
#[cfg(not(feature = "headless"))]
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
fn upload_dirty_chunks(
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

/// Computes which collision tiles a body overlaps based on its rotated AABB.
fn compute_required_tiles(record: &PixelBodyRecord) -> Vec<TilePos> {
  let half_w = record.width as f32 / 2.0;
  let half_h = record.height as f32 / 2.0;
  let (cos_r, sin_r) = (record.rotation.cos(), record.rotation.sin());

  let corners = [
    Vec2::new(-half_w, -half_h),
    Vec2::new(half_w, -half_h),
    Vec2::new(-half_w, half_h),
    Vec2::new(half_w, half_h),
  ];

  let (mut min_x, mut max_x) = (f32::INFINITY, f32::NEG_INFINITY);
  let (mut min_y, mut max_y) = (f32::INFINITY, f32::NEG_INFINITY);

  for c in corners {
    let rotated = Vec2::new(
      c.x * cos_r - c.y * sin_r + record.position.x,
      c.x * sin_r + c.y * cos_r + record.position.y,
    );
    min_x = min_x.min(rotated.x);
    max_x = max_x.max(rotated.x);
    min_y = min_y.min(rotated.y);
    max_y = max_y.max(rotated.y);
  }

  WorldRect::new(
    min_x.floor() as i64,
    min_y.floor() as i64,
    (max_x.ceil() - min_x.floor()) as u32 + 1,
    (max_y.ceil() - min_y.floor()) as u32 + 1,
  )
  .to_tile_range()
  .collect()
}

/// System: Queues pixel bodies when their chunk loads.
///
/// Bodies are not spawned immediately - they wait in `PendingPixelBodies` until
/// their required collision tiles are cached.
fn queue_pixel_bodies_on_chunk_load(
  loaded_chunks: Res<LoadedChunks>,
  save_resource: Option<Res<WorldSaveResource>>,
  mut pending: ResMut<PendingPixelBodies>,
  mut id_generator: ResMut<PixelBodyIdGenerator>,
  mut persistence_tasks: ResMut<PersistenceTasks>,
) {
  if loaded_chunks.positions.is_empty() {
    return;
  }

  let Some(save_resource) = save_resource else {
    return;
  };

  let save = match save_resource.save.read() {
    Ok(s) => s,
    Err(_) => return,
  };

  for &chunk_pos in &loaded_chunks.positions {
    let records = save.load_bodies_for_chunk(chunk_pos);

    for record in records {
      id_generator.ensure_above(record.stable_id);

      // Skip if already pending (prevents duplicate spawning)
      if pending
        .entries
        .iter()
        .any(|e| e.record.stable_id == record.stable_id)
      {
        continue;
      }

      // Check if body is empty (stale record) before queueing
      let body = record.to_pixel_body();
      if body.is_empty() {
        persistence_tasks.queue_body_remove(record.stable_id);
        continue;
      }

      // Compute which collision tiles this body needs
      let required_tiles = compute_required_tiles(&record);

      pending.entries.push(PendingBodyEntry {
        record,
        required_tiles,
      });
    }
  }
}

/// System: Spawns pending pixel bodies when their collision tiles are ready.
///
/// Bodies wait in `PendingPixelBodies` until all required collision tiles are
/// cached, ensuring they don't fall through terrain on load.
fn spawn_pending_pixel_bodies(
  mut commands: Commands,
  mut pending: ResMut<PendingPixelBodies>,
  cache: Res<CollisionCache>,
  mut persistence_tasks: ResMut<PersistenceTasks>,
  existing_bodies: Query<&PixelBodyId>,
) {
  pending.entries.retain(|entry| {
    // Check if all required tiles are cached
    let ready = entry.required_tiles.iter().all(|t| cache.contains(*t));
    if !ready {
      return true; // Keep waiting
    }

    let record = &entry.record;

    // Wait for old entity with same ID to be despawned (deferred despawn)
    // This prevents duplicate bodies when a chunk unloads and reloads in
    // the same frame - the old entity's despawn is applied after this runs.
    if existing_bodies.iter().any(|id| id.0 == record.stable_id) {
      return true; // Keep waiting until old entity is gone
    }

    // All tiles ready and no duplicate - spawn the body
    let body = record.to_pixel_body();

    if body.is_empty() {
      persistence_tasks.queue_body_remove(record.stable_id);
      return false;
    }

    #[cfg(any(feature = "avian2d", feature = "rapier2d"))]
    let Some(collider) = crate::pixel_body::generate_collider(&body) else {
      return false;
    };

    let transform = Transform {
      translation: record.position.extend(0.0),
      rotation: Quat::from_rotation_z(record.rotation),
      scale: Vec3::ONE,
    };

    // Spawn with Dynamic - collision is guaranteed ready
    // Initialize LastBlitTransform with actual transform so erasure detection
    // doesn't skip this body on its first frame (detect_external_erasure skips
    // bodies with None transform).
    #[allow(unused_mut, unused_variables)]
    let mut entity_commands = commands.spawn((
      body,
      LastBlitTransform {
        transform: Some(GlobalTransform::from(transform)),
        written_positions: Vec::new(),
      },
      DisplacementState::default(),
      transform,
      PixelBodyId::new(record.stable_id),
      Persistable,
    ));

    #[cfg(feature = "avian2d")]
    entity_commands.insert((
      collider,
      avian2d::prelude::RigidBody::Dynamic,
      avian2d::prelude::LinearVelocity(record.linear_velocity),
      avian2d::prelude::AngularVelocity(record.angular_velocity),
      crate::collision::CollisionQueryPoint,
      crate::culling::StreamCulled,
    ));

    #[cfg(all(feature = "rapier2d", not(feature = "avian2d")))]
    entity_commands.insert((
      collider,
      bevy_rapier2d::prelude::RigidBody::Dynamic,
      bevy_rapier2d::prelude::Velocity {
        linvel: record.linear_velocity,
        angvel: record.angular_velocity,
      },
      crate::collision::CollisionQueryPoint,
      crate::culling::StreamCulled,
    ));

    false // Remove from pending
  });
}

/// Run condition: Returns true if simulation is not paused.
fn simulation_not_paused(state: Res<SimulationState>) -> bool {
  state.is_running()
}
