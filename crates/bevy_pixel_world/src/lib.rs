//! Pixel World - Infinite cellular automata simulation plugin for Bevy.
//!
//! This crate provides a plugin for simulating infinite cellular automata
//! worlds.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::sprite_render::Material2dPlugin;
use persistence::PersistenceBackend;

pub mod basic_persistence;
pub mod bodies_plugin;
pub mod buoyancy;
pub mod collision;
pub mod coords;
pub mod creative_mode;
pub mod debug_camera;
pub mod debug_controller;
pub mod debug_shim;
pub mod diagnostics;
pub mod material;
pub mod persistence;
pub mod pixel;
pub mod pixel_awareness;
pub mod pixel_body;
pub mod plugin_bundle;
pub mod primitives;
pub mod render;
pub mod schedule;
pub mod scheduling;
pub mod seeding;
pub mod simulation;
pub mod text;
#[cfg(feature = "tracy")]
mod tracy_init;
pub mod visual_debug;
pub mod world;

pub use basic_persistence::BasicPersistencePlugin;
pub use bodies_plugin::PixelBodiesPlugin;
pub use buoyancy::BuoyancyConfig;
pub use buoyancy::SubmersionConfig;
pub use collision::{CollisionCache, CollisionConfig, CollisionQueryPoint, CollisionTasks};
pub use coords::{
  CHUNK_SIZE, ChunkPos, ColorIndex, LocalPos, MaterialId, TILE_SIZE, TilePos, WorldFragment,
  WorldPos, WorldRect,
};
pub use creative_mode::CreativeModePlugins;
pub use debug_camera::{CameraZoom, PixelDebugControllerCameraPlugin};
pub use debug_controller::{BrushState, PixelDebugControllerPlugin, UiPointerState};
pub use material::{Material, Materials, MaterialsConfig, PhysicsState, ids as material_ids};
pub use persistence::{PixelBodyRecord, WorldSave};
pub use pixel::{Pixel, PixelFlags, PixelSurface};
pub use pixel_awareness::GridSampleConfig;
pub use pixel_body::{
  Bomb, BombShellMask, DisplacementState, LastBlitTransform, PendingPixelBody, Persistable,
  PixelBody, PixelBodyId, PixelBodyIdGenerator, PixelBodyLoader, SpawnPixelBody,
  SpawnPixelBodyFromImage, finalize_pending_pixel_bodies, generate_collider, update_pixel_bodies,
};
pub use plugin_bundle::PixelWorldFullBundle;
pub use primitives::{Chunk, Surface};
pub use render::{
  ChunkMaterial, Rgba, create_chunk_quad, create_palette_texture, create_pixel_texture,
  create_texture, materialize, rgb, spawn_static_chunk, upload_palette, upload_pixels,
  upload_surface,
};
pub use schedule::{PixelWorldSet, SimulationPhase};
pub use seeding::{ChunkSeeder, MaterialSeeder, NoiseSeeder, PersistenceSeeder};
pub use simulation::{HeatConfig, simulate_tick};
pub use text::{CpuFont, TextMask, TextStyle, draw_text, rasterize_text, stamp_text};
#[cfg(feature = "tracy")]
pub use tracy_init::init_tracy;
pub use world::control::{
  PersistenceComplete, PersistenceControl, PersistenceFuture, PersistenceHandle,
  RequestPersistence, SimulationState,
};
pub use world::plugin::{SeededChunks, StreamingCamera, UnloadingChunks};
// Re-export culling types from streaming module for backward compatibility
pub use world::streaming::{CullingConfig, StreamCulled};
pub use world::{PixelWorld, PixelWorldBundle, PixelWorldConfig, SpawnPixelWorld};

/// Configuration for chunk persistence.
///
/// Persistence is enabled by providing a path to a save file.
/// When disabled (no path), the world exists only in memory.
///
/// # Example
/// ```ignore
/// // Enable persistence with explicit path
/// let config = PersistenceConfig::at("/home/user/saves/world.save");
///
/// // Disable persistence
/// let config = PersistenceConfig::disabled();
/// ```
#[derive(Clone, Debug, Default)]
pub struct PersistenceConfig {
  /// Path to save file. None = persistence disabled.
  pub path: Option<PathBuf>,
  /// World seed for procedural generation.
  pub world_seed: u64,
}

impl PersistenceConfig {
  /// Creates a persistence config with the given save file path.
  pub fn at(path: impl Into<PathBuf>) -> Self {
    Self {
      path: Some(path.into()),
      world_seed: 42,
    }
  }

  /// Disables persistence (in-memory only).
  pub fn disabled() -> Self {
    Self {
      path: None,
      world_seed: 42,
    }
  }

  /// Sets the world seed.
  pub fn with_seed(mut self, seed: u64) -> Self {
    self.world_seed = seed;
    self
  }

  /// Returns true if persistence is enabled (path is set).
  pub fn is_enabled(&self) -> bool {
    self.path.is_some()
  }
}

/// Plugin for infinite cellular automata simulation.
///
/// This plugin provides:
/// - Chunk material rendering
/// - Automatic chunk streaming based on camera position
/// - Async background seeding
/// - GPU texture upload for dirty chunks
/// - Automatic chunk persistence (when enabled)
/// - Entity culling outside the streaming window (when enabled)
///
/// To use automatic streaming, spawn a `PixelWorldBundle` and mark a camera
/// with `StreamingCamera`.
#[derive(Default)]
pub struct PixelWorldPlugin {
  /// Default configuration for spawned pixel worlds.
  pub config: PixelWorldConfig,
  /// Persistence configuration.
  pub persistence: PersistenceConfig,
  /// Culling configuration.
  pub culling: CullingConfig,
}

impl PixelWorldPlugin {
  /// Sets the persistence configuration.
  pub fn persistence(mut self, config: PersistenceConfig) -> Self {
    self.persistence = config;
    self
  }

  /// Sets the culling configuration.
  pub fn culling(mut self, config: CullingConfig) -> Self {
    self.culling = config;
    self
  }
}

impl Plugin for PixelWorldPlugin {
  fn build(&self, app: &mut App) {
    // Embed the chunk shader and register material (rendering only)
    if app.is_plugin_added::<bevy::render::RenderPlugin>() {
      bevy::asset::embedded_asset!(app, "render/shaders/chunk.wgsl");
      app.add_plugins(Material2dPlugin::<ChunkMaterial>::default());
      app.insert_resource(world::plugin::RenderingEnabled);
    }

    // Initialize Materials registry (users can override by inserting before plugin)
    app.init_resource::<Materials>();

    // Store default config as resource for SpawnPixelWorld
    app.insert_resource(DefaultPixelWorldConfig(self.config.clone()));

    // Store persistence config
    app.insert_resource(DefaultPersistenceConfig(self.persistence.clone()));

    // Store culling config
    app.insert_resource(self.culling.clone());

    // Initialize persistence if a path is configured
    #[cfg(not(target_family = "wasm"))]
    if let Some(ref path) = self.persistence.path {
      // Extract directory and filename from the absolute path
      let base_dir = path
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();
      let file_name = path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("world.save")
        .to_string();

      let backend = match persistence::NativePersistence::new(base_dir.clone()) {
        Ok(b) => std::sync::Arc::new(b),
        Err(e) => {
          error!(
            "Failed to create save directory {:?}: {}. Persistence disabled.",
            base_dir, e
          );
          app.add_plugins(world::plugin::PixelWorldStreamingPlugin);
          if app.is_plugin_added::<bevy::render::RenderPlugin>() {
            app.add_plugins(visual_debug::VisualDebugPlugin);
          }
          return;
        }
      };

      // Open or create the save file
      let save_result = persistence::block_on(
        backend.open_or_create_async(&file_name, self.persistence.world_seed),
      );
      match save_result {
        Ok(save) => {
          info!("Opened world save {:?}", path);
          app.insert_resource(PersistenceControl::with_save(backend, save, path.clone()));
        }
        Err(e) => {
          error!(
            "Failed to open world save {:?}: {}. Persistence disabled.",
            path, e
          );
        }
      }
    }

    // WASM: Persistence uses a dedicated Web Worker for OPFS operations.
    // The Web Worker is required because `createSyncAccessHandle()` only
    // works in Web Workers, not the main browser thread.
    #[cfg(target_family = "wasm")]
    if let Some(ref path) = self.persistence.path {
      let seed = self.persistence.world_seed;

      // Create IoDispatcher which spawns the Web Worker
      let io_dispatcher = persistence::IoDispatcher::new();

      // Send Initialize command to the worker
      io_dispatcher.send(persistence::IoCommand::Initialize {
        path: path.clone(),
        seed,
      });

      app.insert_resource(io_dispatcher);

      // Insert a pending PersistenceControl - will be activated when worker
      // initializes
      app.insert_resource(world::control::PendingPersistenceInit {
        path: path.clone(),
        world_seed: seed,
      });

      info!(
        "WASM persistence enabled via Web Worker, initializing {:?}",
        path
      );
    }

    // Add world streaming systems
    app.add_plugins(world::plugin::PixelWorldStreamingPlugin);

    // Add visual debug plugin (requires rendering infrastructure)
    if app.is_plugin_added::<bevy::render::RenderPlugin>() {
      app.add_plugins(visual_debug::VisualDebugPlugin);
    }
  }
}

/// Resource holding the default configuration for spawned pixel worlds.
#[derive(Resource)]
pub struct DefaultPixelWorldConfig(pub PixelWorldConfig);

/// Resource holding the default persistence configuration.
#[derive(Resource)]
pub struct DefaultPersistenceConfig(pub PersistenceConfig);
