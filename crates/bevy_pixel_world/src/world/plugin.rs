//! ECS plugin and systems for PixelWorld.
//!
//! Provides automatic chunk streaming, seeding, and GPU upload.

use bevy::prelude::*;
// WASM compat: std::time::Instant panics on wasm32
use web_time::Instant;

use super::PixelWorld;
use super::control::{PersistenceComplete, RequestPersistence, SimulationState};
use super::persistence_systems::{
  LoadedChunkDataStore, dispatch_chunk_loads, dispatch_save_task, flush_persistence_queue,
  handle_persistence_messages, notify_persistence_complete, poll_chunk_loads, poll_io_results,
  poll_save_task, process_pending_save_requests,
};
use super::streaming::poll_seeding_tasks;
use super::streaming::{
  CullingConfig, SeedingTasks, clear_chunk_tracking, dispatch_seeding, update_entity_culling,
  update_simulation_bounds, update_streaming_windows,
};
pub use super::streaming::{SeededChunks, StreamingCamera, UnloadingChunks};
pub(crate) use super::streaming::{SharedChunkMesh, SharedPaletteTexture};
use super::systems::upload_dirty_chunks;
use crate::coords::CHUNK_SIZE;
use crate::debug_shim;
use crate::material::Materials;
use crate::persistence::PersistenceTasks;
use crate::persistence::tasks::{LoadingChunks, SavingChunks};
use crate::render::{create_chunk_quad, create_palette_texture, upload_palette};
use crate::schedule::{PixelWorldSet, SimulationPhase};
use crate::simulation;
use crate::simulation::HeatConfig;

/// Marker resource indicating rendering infrastructure is available.
/// Inserted by PixelWorldPlugin when RenderPlugin is detected.
#[derive(Resource)]
pub(crate) struct RenderingEnabled;

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
      .init_resource::<LoadingChunks>()
      .init_resource::<SavingChunks>()
      .init_resource::<LoadedChunkDataStore>()
      .init_resource::<crate::collision::CollisionCache>()
      .init_resource::<CullingConfig>()
      .init_resource::<UnloadingChunks>()
      .init_resource::<SeededChunks>()
      .init_resource::<SimulationState>()
      .init_resource::<crate::diagnostics::SimulationMetrics>()
      .init_resource::<HeatConfig>()
      .add_message::<RequestPersistence>()
      .add_message::<PersistenceComplete>();

    // Configure set ordering: Pre → Sim → Post
    app.configure_sets(
      Update,
      (
        PixelWorldSet::PreSimulation,
        PixelWorldSet::Simulation,
        PixelWorldSet::PostSimulation,
      )
        .chain(),
    );

    // Configure simulation sub-phases
    app.configure_sets(
      Update,
      (
        SimulationPhase::BeforeCATick,
        SimulationPhase::CATick,
        SimulationPhase::AfterCATick,
      )
        .chain()
        .in_set(PixelWorldSet::Simulation),
    );

    app.add_systems(
      PreStartup,
      setup_shared_resources.run_if(resource_exists::<RenderingEnabled>),
    );

    // Core pre-simulation systems (streaming, persistence messages, seeding)
    app.add_systems(
      Update,
      (
        clear_chunk_tracking,
        // Poll I/O worker results (initialization, chunk loads, etc.)
        poll_io_results,
        handle_persistence_messages,
        update_streaming_windows,
        update_entity_culling,
        // Async persistence loading: dispatch loads for new chunks, poll completed loads
        dispatch_chunk_loads,
        poll_chunk_loads,
        // Seeding: dispatch and poll async seeding tasks
        dispatch_seeding,
        poll_seeding_tasks,
        update_simulation_bounds,
      )
        .chain()
        .in_set(PixelWorldSet::PreSimulation),
    );

    // Core simulation system
    app.add_systems(
      Update,
      run_simulation
        .run_if(simulation_not_paused)
        .in_set(SimulationPhase::CATick),
    );

    // Core post-simulation systems (persistence flush)
    app.add_systems(
      Update,
      (
        process_pending_save_requests,
        // Async persistence saving: dispatch save task, poll completion
        dispatch_save_task,
        poll_save_task,
        // Legacy sync flush (for copy-on-write and immediate flushes)
        flush_persistence_queue,
        notify_persistence_complete,
      )
        .chain()
        .in_set(PixelWorldSet::PostSimulation),
    );

    // Render-only systems
    app.add_systems(
      Update,
      (
        initialize_palette
          .after(handle_persistence_messages)
          .before(update_streaming_windows)
          .in_set(PixelWorldSet::PreSimulation),
        upload_dirty_chunks.in_set(PixelWorldSet::PostSimulation),
      )
        .run_if(resource_exists::<RenderingEnabled>),
    );
  }
}

/// Sets up shared resources used by all PixelWorlds.
///
/// Runs in PreStartup to ensure resources are available before user Startup
/// systems that spawn PixelWorlds.
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
  heat_config: Res<HeatConfig>,
  gizmos: debug_shim::GizmosParam,
  mut sim_metrics: ResMut<crate::diagnostics::SimulationMetrics>,
) {
  let Some(materials) = mat_registry else {
    return;
  };

  let debug_gizmos = gizmos.get();

  let start = Instant::now();

  for mut world in worlds.iter_mut() {
    simulation::simulate_tick(&mut world, &materials, debug_gizmos, &heat_config);
  }

  let elapsed_ms = start.elapsed().as_secs_f32() * 1000.0;
  sim_metrics.sim_time.push(elapsed_ms);
}

/// Run condition: Returns true if simulation is not paused.
fn simulation_not_paused(state: Res<SimulationState>) -> bool {
  state.is_running()
}
