//! ECS plugin and systems for PixelWorld.
//!
//! Provides automatic chunk streaming, seeding, and GPU upload.

use bevy::ecs::schedule::ApplyDeferred;
use bevy::prelude::*;

use super::PixelWorld;
use super::body_loader::spawn_pending_pixel_bodies;
use super::control::{
  PersistenceComplete, PersistenceControl, RequestPersistence, SimulationState,
};
use super::persistence_systems::{
  flush_persistence_queue, handle_persistence_messages, notify_persistence_complete,
  process_pending_save_requests, save_pixel_bodies_on_chunk_unload, save_pixel_bodies_on_request,
};
#[cfg(not(feature = "headless"))]
use super::streaming::poll_seeding_tasks;
use super::streaming::{
  CullingConfig, PendingPixelBodies, SeedingTasks, clear_chunk_tracking, dispatch_seeding,
  queue_pixel_bodies_on_chunk_seed, update_entity_culling, update_simulation_bounds,
  update_streaming_windows,
};
pub use super::streaming::{SeededChunks, StreamingCamera, UnloadingChunks};
pub(crate) use super::streaming::{SharedChunkMesh, SharedPaletteTexture};
#[cfg(not(feature = "headless"))]
use super::systems::upload_dirty_chunks;
#[cfg(feature = "visual_debug")]
use crate::collision::draw_collision_gizmos;
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
use crate::collision::physics::{PhysicsColliderRegistry, sync_physics_colliders};
use crate::collision::{
  CollisionCache, CollisionConfig, CollisionTasks, dispatch_collision_tasks,
  invalidate_dirty_tiles, poll_collision_tasks,
};
use crate::coords::CHUNK_SIZE;
use crate::debug_shim;
use crate::material::Materials;
use crate::persistence::PersistenceTasks;
use crate::pixel_body::{
  PixelBodyIdGenerator, apply_readback_changes, detect_external_erasure,
  finalize_pending_pixel_bodies, readback_pixel_bodies, split_pixel_bodies, update_pixel_bodies,
};
#[cfg(not(feature = "headless"))]
use crate::render::{create_chunk_quad, create_palette_texture, upload_palette};
use crate::simulation;

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
      .init_resource::<SeededChunks>()
      .init_resource::<PendingPixelBodies>()
      .init_resource::<PixelBodyIdGenerator>()
      .init_resource::<SimulationState>()
      .init_resource::<PersistenceControl>()
      .add_message::<RequestPersistence>()
      .add_message::<PersistenceComplete>();

    #[cfg(not(feature = "headless"))]
    app.add_systems(PreStartup, setup_shared_resources);

    #[cfg(any(feature = "avian2d", feature = "rapier2d"))]
    {
      app.init_resource::<PhysicsColliderRegistry>();
      app.add_systems(Update, sync_physics_colliders.after(poll_collision_tasks));
    }

    // Core update loop: pre-simulation → barrier → simulation → post-simulation.
    // Render-only systems (palette init, async seeding poll, GPU upload) are added
    // separately below with ordering constraints, avoiding schedule duplication.
    app.add_systems(
      Update,
      (
        // Pre-simulation group
        (
          clear_chunk_tracking,
          handle_persistence_messages,
          update_streaming_windows,
          save_pixel_bodies_on_chunk_unload,
          update_entity_culling,
          dispatch_seeding,
          queue_pixel_bodies_on_chunk_seed,
          update_simulation_bounds,
          finalize_pending_pixel_bodies,
        )
          .chain(),
        // Apply deferred commands so new bodies are visible to simulation
        ApplyDeferred,
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

    // Render-only systems slotted into the shared schedule via ordering
    // constraints.
    #[cfg(not(feature = "headless"))]
    app.add_systems(
      Update,
      (
        initialize_palette
          .after(handle_persistence_messages)
          .before(update_streaming_windows),
        poll_seeding_tasks
          .after(dispatch_seeding)
          .before(queue_pixel_bodies_on_chunk_seed),
        upload_dirty_chunks
          .after(spawn_pending_pixel_bodies)
          .before(process_pending_save_requests),
      ),
    );

    #[cfg(all(not(feature = "headless"), feature = "visual_debug"))]
    app.add_systems(PostUpdate, draw_collision_gizmos);
  }
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

/// Run condition: Returns true if simulation is not paused.
fn simulation_not_paused(state: Res<SimulationState>) -> bool {
  state.is_running()
}
