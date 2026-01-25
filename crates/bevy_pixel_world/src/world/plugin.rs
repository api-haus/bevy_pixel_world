//! ECS plugin and systems for PixelWorld.
//!
//! Provides automatic chunk streaming, seeding, and GPU upload.

use std::collections::HashSet;
use std::sync::atomic::Ordering;

use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::prelude::*;
#[cfg(not(feature = "headless"))]
use bevy::tasks::{AsyncComputeTaskPool, Task};

use super::control::{
  PersistenceComplete, PersistenceControl, RequestPersistence, SimulationState,
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
use crate::coords::{CHUNK_SIZE, ChunkPos, WorldPos, WorldRect};
use crate::culling::{CullingConfig, update_entity_culling};
use crate::debug_shim;
use crate::material::Materials;
use crate::persistence::{
  PersistenceTasks, PixelBodyRecord, WorldSaveResource, compression::compress_lz4,
  format::StorageType,
};
use crate::pixel_body::{
  DisplacementState, LastBlitTransform, Persistable, PixelBody, PixelBodyId, PixelBodyIdGenerator,
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

/// Tracks chunks unloading this frame.
///
/// Populated by `tick_pixel_worlds` before pixel body save systems run.
/// Cleared at the start of each frame.
#[derive(Resource, Default)]
pub struct UnloadingChunks {
  /// Positions of chunks being unloaded.
  pub positions: Vec<ChunkPos>,
}

/// Tracks chunks that finished loading this frame.
///
/// Populated by `poll_seeding_tasks` when seeding completes.
/// Cleared at the start of each frame.
#[derive(Resource, Default)]
pub struct LoadedChunks {
  /// Positions of chunks that just finished loading.
  pub positions: Vec<ChunkPos>,
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
          load_pixel_bodies_on_chunk_load,
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
          load_pixel_bodies_on_chunk_load,
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

// TODO: Extract shared seeding logic (Chunk::new, seeder.seed) into helper
/// System: Dispatches async seeding tasks for unseeded chunks.
#[cfg(not(feature = "headless"))]
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
fn dispatch_seeding(
  mut seeding_tasks: ResMut<SeedingTasks>,
  mut worlds: Query<(Entity, &mut PixelWorld)>,
) {
  let task_pool = AsyncComputeTaskPool::get();

  for (world_entity, world) in worlds.iter_mut() {
    // Count existing tasks and collect in-flight slots in one pass
    let mut in_flight = 0;
    let mut in_flight_slots = std::collections::HashSet::new();
    for task in seeding_tasks.tasks.iter() {
      if task.world_entity == world_entity {
        in_flight += 1;
        in_flight_slots.insert(task.slot_index);
      }
    }

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

      // Spawn async seeding task
      let seeder = world.seeder().clone();
      let task = task_pool.spawn(async move { seed_chunk(seeder.as_ref(), pos) });

      seeding_tasks.tasks.push(SeedingTask {
        world_entity,
        slot_index: slot_idx,
        pos,
        task,
      });

      // Respect concurrency limit
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
      let chunk = seed_chunk(world.seeder().as_ref(), pos);

      // Copy seeded data into slot
      let slot = world.slot_mut(slot_idx);
      slot.chunk.pixels = chunk.pixels;
      slot.chunk.set_all_dirty_rects_full();
      slot.lifecycle = super::slot::ChunkLifecycle::Active;
      slot.dirty = true;

      // If loaded from disk, mark as persisted
      if chunk.from_persistence {
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
      slot.chunk.pixels = seeded_chunk.pixels;
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

/// System: Flushes pending persistence tasks to disk.
///
/// Writes queued chunk and body saves to the save file. Only runs if a
/// WorldSaveResource is present.
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
fn flush_persistence_queue(
  mut persistence_tasks: ResMut<PersistenceTasks>,
  save_resource: Option<ResMut<WorldSaveResource>>,
) {
  let has_chunk_saves = !persistence_tasks.save_queue.is_empty();
  let has_body_saves = !persistence_tasks.body_save_queue.is_empty();

  if !has_chunk_saves && !has_body_saves {
    return;
  }

  let Some(save_resource) = save_resource else {
    // No save file configured, discard queued saves
    persistence_tasks.save_queue.clear();
    persistence_tasks.body_save_queue.clear();
    return;
  };

  // Process all queued saves
  let mut save = match save_resource.save.write() {
    Ok(s) => s,
    Err(e) => {
      eprintln!("Warning: failed to acquire save lock: {}", e);
      persistence_tasks.save_queue.clear();
      persistence_tasks.body_save_queue.clear();
      return;
    }
  };

  // Save chunks
  for task in persistence_tasks.save_queue.drain(..) {
    // Create page table entry and write data
    let entry = crate::persistence::format::PageTableEntry::new(
      task.pos,
      save.data_write_pos + 4, // Skip size prefix
      task.data.len() as u32,
      task.storage_type,
    );

    // Open file and write
    if let Err(e) = write_chunk_data(&save.path, save.data_write_pos, &task.data) {
      eprintln!("Warning: failed to save chunk {:?}: {}", task.pos, e);
      continue;
    }

    // Update save state
    save.index.insert(entry);
    save.data_write_pos += 4 + task.data.len() as u64;
    save.header.chunk_count = save.index.len() as u32;
    save.dirty = true;
  }

  // Save pixel bodies
  for task in persistence_tasks.body_save_queue.drain(..) {
    if let Err(e) = save.save_body(&task.record) {
      eprintln!(
        "Warning: failed to save pixel body {}: {}",
        task.record.stable_id, e
      );
    }
  }

  // Flush page table periodically (every N chunks or on demand)
  if save.dirty
    && let Err(e) = save.flush()
  {
    eprintln!("Warning: failed to flush save: {}", e);
  }
}

/// System: Clears chunk tracking resources at the start of each frame.
fn clear_chunk_tracking(mut unloading: ResMut<UnloadingChunks>, mut loaded: ResMut<LoadedChunks>) {
  unloading.positions.clear();
  loaded.positions.clear();
}

/// Creates a PixelBodyRecord from entity components with blitted transform.
///
/// Shared helper for save systems to avoid duplicating cfg-conditional velocity
/// extraction logic.
#[allow(unused_variables)]
fn create_body_record_blitted(
  entity: Entity,
  body_id: &PixelBodyId,
  body: &PixelBody,
  blitted: &LastBlitTransform,
  #[cfg(feature = "avian2d")] velocities: &Query<(
    Option<&avian2d::prelude::LinearVelocity>,
    Option<&avian2d::prelude::AngularVelocity>,
  )>,
  #[cfg(all(feature = "rapier2d", not(feature = "avian2d")))] velocities: &Query<
    Option<&bevy_rapier2d::prelude::Velocity>,
  >,
) -> Option<PixelBodyRecord> {
  #[cfg(feature = "avian2d")]
  let (lin_vel, ang_vel) = velocities.get(entity).unwrap_or((None, None));

  #[cfg(all(feature = "rapier2d", not(feature = "avian2d")))]
  let velocity = velocities.get(entity).ok().flatten();

  PixelBodyRecord::from_components_blitted(
    body_id,
    body,
    blitted,
    #[cfg(feature = "avian2d")]
    lin_vel,
    #[cfg(feature = "avian2d")]
    ang_vel,
    #[cfg(all(feature = "rapier2d", not(feature = "avian2d")))]
    velocity,
    Vec::new(),
  )
}

/// System: Saves pixel bodies when their chunk unloads.
///
/// Uses the blitted transform to ensure saved position matches where pixels
/// were written.
#[allow(unused_variables)]
fn save_pixel_bodies_on_chunk_unload(
  mut commands: Commands,
  unloading_chunks: Res<UnloadingChunks>,
  mut persistence_tasks: ResMut<PersistenceTasks>,
  bodies: Query<(
    Entity,
    &PixelBodyId,
    &PixelBody,
    &Persistable,
    &LastBlitTransform,
  )>,
  #[cfg(feature = "avian2d")] velocities: Query<(
    Option<&avian2d::prelude::LinearVelocity>,
    Option<&avian2d::prelude::AngularVelocity>,
  )>,
  #[cfg(all(feature = "rapier2d", not(feature = "avian2d")))] velocities: Query<
    Option<&bevy_rapier2d::prelude::Velocity>,
  >,
) {
  if unloading_chunks.positions.is_empty() {
    return;
  }

  let unloading_set: HashSet<_> = unloading_chunks.positions.iter().copied().collect();

  for (entity, body_id, body, _, blitted) in bodies.iter() {
    let Some(bt) = &blitted.transform else {
      continue;
    };

    let (chunk_pos, _) =
      WorldPos::new(bt.translation().x as i64, bt.translation().y as i64).to_chunk_and_local();

    if !unloading_set.contains(&chunk_pos) {
      continue;
    }

    let Some(record) = create_body_record_blitted(
      entity,
      body_id,
      body,
      blitted,
      #[cfg(any(
        feature = "avian2d",
        all(feature = "rapier2d", not(feature = "avian2d"))
      ))]
      &velocities,
    ) else {
      continue;
    };

    persistence_tasks.queue_body_save(record);
    commands.entity(entity).despawn();
  }
}

/// System: Loads pixel bodies when their chunk loads.
fn load_pixel_bodies_on_chunk_load(
  mut commands: Commands,
  loaded_chunks: Res<LoadedChunks>,
  save_resource: Option<Res<WorldSaveResource>>,
  mut id_generator: ResMut<PixelBodyIdGenerator>,
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

      let body = record.to_pixel_body();

      #[cfg(any(feature = "avian2d", feature = "rapier2d"))]
      let Some(collider) = crate::pixel_body::generate_collider(&body) else {
        continue;
      };

      let transform = Transform {
        translation: record.position.extend(0.0),
        rotation: Quat::from_rotation_z(record.rotation),
        scale: Vec3::ONE,
      };

      #[allow(unused_mut, unused_variables)]
      let mut entity_commands = commands.spawn((
        body,
        LastBlitTransform::default(),
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
    }
  }
}

/// Writes chunk data to the save file at the given offset.
fn write_chunk_data(path: &std::path::Path, offset: u64, data: &[u8]) -> std::io::Result<()> {
  use std::io::{Seek, SeekFrom, Write};

  let mut file = std::fs::File::options().write(true).open(path)?;
  file.seek(SeekFrom::Start(offset))?;

  // Write size prefix
  let size_bytes = (data.len() as u32).to_le_bytes();
  file.write_all(&size_bytes)?;
  file.write_all(data)?;

  Ok(())
}

/// Run condition: Returns true if simulation is not paused.
fn simulation_not_paused(state: Res<SimulationState>) -> bool {
  state.is_running()
}

/// System: Ticks the auto-save timer and requests saves when the interval
/// elapses.
fn tick_auto_save_timer(time: Res<Time>, mut persistence: ResMut<PersistenceControl>) {
  if !persistence.auto_save.enabled {
    return;
  }

  persistence.time_since_save += time.delta();

  if persistence.time_since_save >= persistence.auto_save.interval {
    persistence.request_save();
    persistence.reset_auto_save_timer();
  }
}

/// System: Converts `RequestPersistence` messages into pending save requests.
fn handle_persistence_messages(
  mut messages: MessageReader<RequestPersistence>,
  mut persistence: ResMut<PersistenceControl>,
) {
  for message in messages.read() {
    if message.include_bodies {
      persistence.request_save();
    } else {
      persistence.request_chunk_save();
    }
  }
}

/// System: Processes pending save requests by queuing all modified chunks.
///
/// When a save is requested (via `PersistenceControl::request_save()` or
/// auto-save), this system queues all modified chunks to `PersistenceTasks` so
/// they get written by `flush_persistence_queue`.
fn process_pending_save_requests(
  persistence: Res<PersistenceControl>,
  mut persistence_tasks: ResMut<PersistenceTasks>,
  mut worlds: Query<&mut PixelWorld>,
) {
  if persistence.pending_requests.is_empty() {
    return;
  }

  let mut total_saved = 0;

  // Queue all modified chunks for saving
  for mut world in worlds.iter_mut() {
    // Collect chunks that need saving
    let to_save: Vec<_> = world
      .active_chunks()
      .filter_map(|(pos, idx)| {
        let slot = world.slot(idx);
        if slot.needs_save() {
          Some((pos, idx))
        } else {
          None
        }
      })
      .collect();

    // Queue each chunk and mark as persisted
    for (pos, idx) in to_save {
      let slot = world.slot(idx);
      let compressed = compress_lz4(slot.chunk.pixels.as_bytes());
      persistence_tasks.queue_save(pos, compressed, StorageType::Full);

      // Mark slot as persisted so we don't save again until modified
      let slot = world.slot_mut(idx);
      slot.persisted = true;
      total_saved += 1;
    }
  }

  if total_saved > 0 {
    info!("Queued {} chunks for saving", total_saved);
  }
}

/// System: Saves all pixel bodies when a full save is requested.
///
/// Unlike `save_pixel_bodies_on_chunk_unload`, this saves ALL bodies without
/// despawning them, used for manual saves (Ctrl+S) and auto-saves.
#[allow(unused_variables)]
fn save_pixel_bodies_on_request(
  persistence: Res<PersistenceControl>,
  mut persistence_tasks: ResMut<PersistenceTasks>,
  bodies: Query<(
    Entity,
    &PixelBodyId,
    &PixelBody,
    &Persistable,
    &LastBlitTransform,
  )>,
  #[cfg(feature = "avian2d")] velocities: Query<(
    Option<&avian2d::prelude::LinearVelocity>,
    Option<&avian2d::prelude::AngularVelocity>,
  )>,
  #[cfg(all(feature = "rapier2d", not(feature = "avian2d")))] velocities: Query<
    Option<&bevy_rapier2d::prelude::Velocity>,
  >,
) {
  let save_bodies = persistence
    .pending_requests
    .iter()
    .any(|req| req.include_bodies);

  if !save_bodies {
    return;
  }

  let mut count = 0;
  for (entity, body_id, body, _, blitted) in bodies.iter() {
    let Some(record) = create_body_record_blitted(
      entity,
      body_id,
      body,
      blitted,
      #[cfg(any(
        feature = "avian2d",
        all(feature = "rapier2d", not(feature = "avian2d"))
      ))]
      &velocities,
    ) else {
      continue;
    };

    persistence_tasks.queue_body_save(record);
    count += 1;
  }

  if count > 0 {
    info!("Queued {} pixel bodies for saving", count);
  }
}

/// System: Notifies pending save requests that they have completed.
///
/// Runs after `flush_persistence_queue` to mark handles as complete and emit
/// messages.
fn notify_persistence_complete(
  mut persistence: ResMut<PersistenceControl>,
  mut complete_messages: MessageWriter<PersistenceComplete>,
) {
  for request in persistence.pending_requests.drain(..) {
    request.completed.store(true, Ordering::Release);
    complete_messages.write(PersistenceComplete {
      request_id: request.id,
      success: true,
      error: None,
    });
  }
}
