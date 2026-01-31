//! ECS plugin and systems for PixelWorld.
//!
//! Provides automatic chunk streaming, seeding, and GPU upload.

use bevy::prelude::*;
// WASM compat: std::time::Instant panics on wasm32
use web_time::Instant;

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
use super::{
  PersistenceInitialized, PixelWorld, WorldInitState, WorldLoadingProgress, WorldReady,
  world_is_ready,
};
use crate::coords::CHUNK_SIZE;
use crate::debug_shim;
use crate::material::Materials;
use crate::persistence::PersistenceTasks;
use crate::persistence::io_worker::IoDispatcher;
use crate::persistence::tasks::{LoadingChunks, SavingChunks};
use crate::render::{create_chunk_quad, create_palette_texture, upload_palette};
use crate::schedule::{PixelWorldSet, SimulationPhase};
use crate::simulation;
use crate::simulation::HeatConfig;

/// Marker resource indicating rendering infrastructure is available.
/// Inserted by PixelWorldPlugin when RenderPlugin is detected.
#[derive(Resource)]
pub(crate) struct RenderingEnabled;

/// Controls how async tasks are polled.
///
/// - `Block`: Block until tasks complete (synchronous)
/// - `Poll`: Check completion without blocking (async)
///
/// Default when absent:
/// - With `RenderingEnabled`: Poll (async)
/// - Without `RenderingEnabled`: Block (backwards compatibility)
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AsyncTaskBehavior {
  #[default]
  Block,
  Poll,
}

/// Returns true if async tasks should block until complete.
pub(crate) fn should_block_tasks(
  rendering: Option<Res<RenderingEnabled>>,
  async_behavior: Option<Res<AsyncTaskBehavior>>,
) -> bool {
  match async_behavior.map(|r| *r) {
    Some(AsyncTaskBehavior::Block) => true,
    Some(AsyncTaskBehavior::Poll) => false,
    None => rendering.is_none(), // backwards compat
  }
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
      // World initialization state tracking
      .init_resource::<WorldInitState>()
      .init_resource::<WorldLoadingProgress>()
      .add_message::<PersistenceInitialized>()
      .add_message::<WorldReady>()
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
        // Update world init state based on persistence readiness
        transition_to_loading_chunks,
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
        // Update loading progress and check for world ready
        update_loading_progress,
        transition_to_ready,
      )
        .chain()
        .in_set(PixelWorldSet::PreSimulation),
    );

    // Core simulation system - only runs when world is ready
    app.add_systems(
      Update,
      run_simulation
        .run_if(simulation_not_paused)
        .run_if(world_is_ready)
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

// ============================================================================
// World Initialization State Systems
// ============================================================================

/// System: Transitions from `Initializing` to `LoadingChunks` when persistence
/// is ready.
///
/// This runs after `poll_io_results` which sets `IoDispatcher::is_ready()`.
fn transition_to_loading_chunks(
  io_dispatcher: Option<Res<IoDispatcher>>,
  mut state: ResMut<WorldInitState>,
  mut events: bevy::ecs::message::MessageWriter<PersistenceInitialized>,
) {
  if *state != WorldInitState::Initializing {
    return;
  }

  let Some(ref dispatcher) = io_dispatcher else {
    return;
  };

  if dispatcher.is_ready() {
    *state = WorldInitState::LoadingChunks;

    // Emit PersistenceInitialized event with counts from IoDispatcher
    let (chunk_count, body_count) = dispatcher.init_counts();
    events.write(PersistenceInitialized {
      chunk_count,
      body_count,
    });

    info!("World state: Initializing -> LoadingChunks");
  }
}

/// System: Transitions from `LoadingChunks` to `Ready` when initial chunks are
/// loaded.
///
/// The world is ready when:
/// 1. There is at least one active chunk (not loading/seeding)
/// 2. No chunks are being loaded from disk
/// 3. No chunks are being seeded
fn transition_to_ready(
  mut state: ResMut<WorldInitState>,
  loading: Res<LoadingChunks>,
  seeding_tasks: Res<SeedingTasks>,
  worlds: Query<&PixelWorld>,
  mut events: bevy::ecs::message::MessageWriter<WorldReady>,
) {
  if *state != WorldInitState::LoadingChunks {
    return;
  }

  // Check if any world has active chunks and nothing is in-flight
  for world in &worlds {
    let has_active_chunks = world.active_count() > 0;
    let no_loading = loading.is_empty();
    let no_seeding = seeding_tasks.is_empty();

    // Count how many chunks are actually active (not loading/seeding)
    let active_chunk_count = world
      .active_chunks()
      .filter(|(_, idx)| world.slot(*idx).is_seeded())
      .count();

    if has_active_chunks && no_loading && no_seeding && active_chunk_count > 0 {
      *state = WorldInitState::Ready;
      events.write(WorldReady);
      info!(
        "World state: LoadingChunks -> Ready ({} active chunks)",
        active_chunk_count
      );
      return;
    }
  }
}

/// System: Updates the loading progress metrics.
fn update_loading_progress(
  mut progress: ResMut<WorldLoadingProgress>,
  state: Res<WorldInitState>,
  io_dispatcher: Option<Res<IoDispatcher>>,
  loading: Res<LoadingChunks>,
  seeding_tasks: Res<SeedingTasks>,
  worlds: Query<&PixelWorld>,
) {
  progress.state = *state;
  progress.persistence_ready = io_dispatcher
    .as_ref()
    .map(|d| d.is_ready())
    .unwrap_or(false);
  progress.chunks_loading = loading.len();
  progress.chunks_seeding = seeding_tasks.len();

  let (mut ready, mut total) = (0, 0);
  for world in &worlds {
    for (_, slot_idx) in world.active_chunks() {
      total += 1;
      if world.slot(slot_idx).is_seeded() {
        ready += 1;
      }
    }
  }
  progress.chunks_ready = ready;
  progress.chunks_total = total;
}
