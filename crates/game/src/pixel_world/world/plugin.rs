//! ECS plugin and systems for PixelWorld.
//!
//! Provides automatic chunk streaming, seeding, and GPU upload.

use bevy::prelude::*;
// WASM compat: std::time::Instant panics on wasm32
use web_time::Instant;

use super::control::{
  ClearPersistence, FreshReseedAllChunks, PersistenceComplete, ReloadAllChunks, RequestPersistence,
  ReseedAllChunks, SimulationState, UpdateSeeder,
};
use super::persistence_systems::{
  LoadedChunkDataStore, dispatch_chunk_loads, dispatch_save_task, flush_persistence_queue,
  handle_clear_persistence, handle_persistence_messages, notify_persistence_complete,
  poll_chunk_loads, poll_io_results, poll_save_task, process_pending_save_requests,
};
use super::streaming::poll_seeding_tasks;
use super::streaming::{
  CullingConfig, SeedingTasks, clear_chunk_tracking, dispatch_seeding, handle_fresh_reseed_request,
  handle_reload_request, handle_reseed_request, handle_update_seeder, update_entity_culling,
  update_simulation_bounds, update_streaming_windows,
};
pub use super::streaming::{SeededChunks, StreamingCamera, UnloadingChunks};
pub(crate) use super::streaming::{SharedChunkMesh, SharedPaletteTexture};
use super::systems::upload_dirty_chunks;
use super::{
  PersistenceInitialized, PixelWorld, WorldInitState, WorldLoadingProgress, WorldReady,
  world_is_ready,
};
use crate::pixel_world::coords::CHUNK_SIZE;
use crate::pixel_world::debug_shim;
use crate::pixel_world::material::Materials;
#[cfg(not(target_family = "wasm"))]
use crate::pixel_world::palette::save_lut_to_bytes;
use crate::pixel_world::palette::{
  GlobalPalette, LUT_CACHE_PATH, LutCacheAsset, PaletteConfig, PaletteSource, colors_from_hex,
  colors_from_image, load_lut_from_bytes,
};
use crate::pixel_world::persistence::PersistenceTasks;
use crate::pixel_world::persistence::io_worker::IoDispatcher;
use crate::pixel_world::persistence::tasks::{LoadingChunks, SavingChunks};
use crate::pixel_world::render::create_chunk_quad;
use crate::pixel_world::schedule::{PixelWorldSet, SimulationPhase};
use crate::pixel_world::simulation;
use crate::pixel_world::simulation::{HeatConfig, SimulationConfig};

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
      .init_resource::<crate::pixel_world::collision::CollisionCache>()
      .init_resource::<CullingConfig>()
      .init_resource::<UnloadingChunks>()
      .init_resource::<SeededChunks>()
      .init_resource::<SimulationState>()
      .init_resource::<crate::pixel_world::diagnostics::SimulationMetrics>()
      .init_resource::<SimulationConfig>()
      .init_resource::<HeatConfig>()
      // World initialization state tracking
      .init_resource::<WorldInitState>()
      .init_resource::<WorldLoadingProgress>()
      .add_message::<PersistenceInitialized>()
      .add_message::<WorldReady>()
      .add_message::<RequestPersistence>()
      .add_message::<PersistenceComplete>()
      .add_message::<ReseedAllChunks>()
      .add_message::<ReloadAllChunks>()
      .add_message::<ClearPersistence>()
      .add_message::<UpdateSeeder>()
      .add_message::<FreshReseedAllChunks>();

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
        // Handle seeder update, reseed/reload/clear requests before dispatching new seeding tasks
        handle_update_seeder,
        handle_reseed_request,
        handle_fresh_reseed_request,
        handle_reload_request,
        handle_clear_persistence,
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

    // Palette hot-reload system (runs always to handle config changes)
    app.add_systems(
      Update,
      watch_palette_config
        .after(handle_persistence_messages)
        .before(update_streaming_windows)
        .in_set(PixelWorldSet::PreSimulation),
    );

    // LUT polling system - runs after watch_palette_config
    app.add_systems(
      Update,
      poll_lut_task
        .after(watch_palette_config)
        .before(update_streaming_windows)
        .in_set(PixelWorldSet::PreSimulation),
    );

    // Render-only systems
    app.add_systems(
      Update,
      (
        upload_palette_if_dirty
          .after(watch_palette_config)
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

  let palette_texture = {
    let mut images = world.resource_mut::<Assets<Image>>();
    crate::pixel_world::palette::create_palette_texture(&mut images)
  };
  world.insert_resource(SharedPaletteTexture {
    handle: palette_texture,
    initialized: false,
  });
}

/// System: Initializes or updates the palette texture when GlobalPalette is
/// dirty.
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
fn upload_palette_if_dirty(
  mut palette_texture: ResMut<SharedPaletteTexture>,
  mut images: ResMut<Assets<Image>>,
  mut global_palette: Option<ResMut<GlobalPalette>>,
) {
  let Some(ref mut global_palette) = global_palette else {
    return;
  };

  // Check if palette needs upload (dirty flag or not yet initialized)
  if !global_palette.dirty && palette_texture.initialized {
    return;
  }

  if let Some(image) = images.get_mut(&palette_texture.handle) {
    crate::pixel_world::palette::upload_palette(global_palette.as_ref(), image);
    global_palette.dirty = false;
    palette_texture.initialized = true;
  }
}

/// System: Watches for PaletteConfig asset changes and rebuilds the palette.
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
fn watch_palette_config(
  mut global_palette: Option<ResMut<GlobalPalette>>,
  mut events: bevy::ecs::message::MessageReader<AssetEvent<PaletteConfig>>,
  palette_configs: Res<Assets<PaletteConfig>>,
  images: Res<Assets<Image>>,
  asset_server: Res<AssetServer>,
) {
  let Some(ref mut global_palette) = global_palette else {
    return;
  };

  for event in events.read() {
    if let AssetEvent::Modified { id } | AssetEvent::LoadedWithDependencies { id } = event {
      // Check if this is our config
      if global_palette
        .config_handle
        .as_ref()
        .is_some_and(|h| h.id() != *id)
      {
        continue;
      }

      // Reload the config
      if let Some(config) = palette_configs.get(*id) {
        let colors = match &config.palette {
          PaletteSource::Colors { colors } => colors_from_hex(colors),
          PaletteSource::Image { image } => {
            // Load the palette image
            let image_handle: Handle<Image> = asset_server.load(image);
            if let Some(img) = images.get(&image_handle) {
              colors_from_image(img)
            } else {
              // Image not loaded yet, will retry on next event
              continue;
            }
          }
        };

        global_palette.colors = colors;
        global_palette.lut_config = config.lut.clone();
        global_palette.start_lut_build();
        global_palette.dirty = true;
        info!("Palette reloaded from config (async LUT rebuild started)");
      }
    }
  }
}

/// Tracks whether we've attempted to load the cached LUT this session.
#[derive(Default)]
struct LutCacheState {
  load_attempted: bool,
  #[cfg(not(target_family = "wasm"))]
  save_needed: bool,
}

/// System: Polls the LUT build task for completion.
///
/// On first run, attempts to load cached LUT from `assets/lut.bin.lz4`.
/// If cache is valid (hash matches), uses it directly.
/// If cache is missing/invalid, rebuilds synchronously and saves (native only).
/// Hot-reload rebuilds use async to keep the old LUT usable during rebuild.
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
fn poll_lut_task(
  mut global_palette: Option<ResMut<GlobalPalette>>,
  rendering: Option<Res<RenderingEnabled>>,
  async_behavior: Option<Res<AsyncTaskBehavior>>,
  asset_server: Option<Res<AssetServer>>,
  lut_assets: Option<Res<Assets<LutCacheAsset>>>,
  mut lut_cache_handle: Local<Option<Handle<LutCacheAsset>>>,
  mut cache_state: Local<LutCacheState>,
) {
  let Some(ref mut palette) = global_palette else {
    return;
  };

  // Initial build (no LUT exists yet)
  if !palette.lut_ready() && !palette.lut_building() {
    let expected_hash = palette.compute_hash();

    // Try loading cached LUT via Bevy asset system
    if !cache_state.load_attempted {
      cache_state.load_attempted = true;

      if let Some(ref server) = asset_server {
        let handle: Handle<LutCacheAsset> = server.load(LUT_CACHE_PATH);
        *lut_cache_handle = Some(handle);
        // Don't block here - we'll check on the next frame
        return;
      }
    }

    // Check if cached LUT is loaded and valid
    if let Some(ref handle) = *lut_cache_handle {
      if let Some(ref assets) = lut_assets {
        match asset_server.as_ref().map(|s| s.load_state(handle)) {
          Some(bevy::asset::LoadState::Loaded) => {
            if let Some(cache) = assets.get(handle) {
              if let Some((lut, cached_hash)) = load_lut_from_bytes(&cache.0) {
                if cached_hash == expected_hash {
                  palette.set_lut_from_cache(lut, cached_hash);
                  info!("LUT loaded from cache (hash matched)");
                  *lut_cache_handle = None;
                  return;
                }
                info!("LUT cache hash mismatch, rebuilding");
              } else {
                warn!("LUT cache corrupted, rebuilding");
              }
            }
            *lut_cache_handle = None;
          }
          Some(bevy::asset::LoadState::Failed(_)) => {
            // Cache doesn't exist or failed to load - that's fine, we'll build it
            debug!("LUT cache not found, building");
            *lut_cache_handle = None;
          }
          Some(bevy::asset::LoadState::Loading) | Some(bevy::asset::LoadState::NotLoaded) => {
            // Still loading, wait
            return;
          }
          None => {
            *lut_cache_handle = None;
          }
        }
      }
    }

    // Cache miss or invalid - rebuild synchronously
    let config = palette.lut_config.clone();
    palette.rebuild_lut(config);
    info!("LUT built synchronously (initial)");

    // Mark for saving on native
    #[cfg(not(target_family = "wasm"))]
    {
      cache_state.save_needed = true;
    }

    return;
  }

  // Save LUT cache after rebuild (native only)
  #[cfg(not(target_family = "wasm"))]
  if cache_state.save_needed && palette.lut_ready() {
    cache_state.save_needed = false;
    if let Some(lut_data) = palette.lut_data() {
      let hash = palette.compute_hash();
      let bytes = save_lut_to_bytes(lut_data, hash);

      // Save to assets directory
      let assets_path = std::path::Path::new("assets").join(LUT_CACHE_PATH);
      match std::fs::write(&assets_path, &bytes) {
        Ok(()) => info!(
          "LUT cache saved to {} ({} bytes compressed)",
          assets_path.display(),
          bytes.len()
        ),
        Err(e) => warn!("Failed to save LUT cache: {}", e),
      }
    }
  }

  // Poll pending async rebuild (from hot-reload)
  let block = should_block_tasks(rendering, async_behavior);
  if palette.poll_lut(block) {
    info!("LUT async rebuild completed");

    // Save updated cache on native
    #[cfg(not(target_family = "wasm"))]
    {
      cache_state.save_needed = true;
    }
  }
}

/// System: Runs cellular automata simulation on all pixel worlds.
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
fn run_simulation(
  mut worlds: Query<&mut PixelWorld>,
  mat_registry: Option<Res<Materials>>,
  sim_config: Res<SimulationConfig>,
  heat_config: Res<HeatConfig>,
  gizmos: debug_shim::GizmosParam,
  mut sim_metrics: ResMut<crate::pixel_world::diagnostics::SimulationMetrics>,
) {
  let Some(materials) = mat_registry else {
    return;
  };

  let debug_gizmos = gizmos.get();

  let start = Instant::now();

  for mut world in worlds.iter_mut() {
    simulation::simulate_tick(
      &mut world,
      &materials,
      debug_gizmos,
      &sim_config,
      &heat_config,
    );
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
