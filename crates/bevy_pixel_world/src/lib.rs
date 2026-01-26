//! Pixel World - Infinite cellular automata simulation plugin for Bevy.
//!
//! This crate provides a plugin for simulating infinite cellular automata
//! worlds.

use std::path::PathBuf;

use bevy::prelude::*;
#[cfg(not(feature = "headless"))]
use bevy::sprite_render::Material2dPlugin;

#[cfg(all(feature = "buoyancy", any(feature = "avian2d", feature = "rapier2d")))]
pub mod buoyancy;
pub mod collision;
pub mod coords;
pub mod culling;
pub mod debug_shim;
#[cfg(feature = "diagnostics")]
pub mod diagnostics;
pub mod material;
pub mod persistence;
pub mod pixel;
pub mod pixel_body;
pub mod primitives;
pub mod render;
pub mod scheduling;
pub mod seeding;
pub mod simulation;
#[cfg(feature = "submergence")]
pub mod submergence;
pub mod text;
#[cfg(feature = "tracy")]
mod tracy_init;
#[cfg(feature = "visual_debug")]
pub mod visual_debug;
pub mod world;

pub use collision::{CollisionCache, CollisionConfig, CollisionQueryPoint, CollisionTasks};
pub use coords::{
  CHUNK_SIZE, ChunkPos, ColorIndex, LocalPos, MaterialId, TILE_SIZE, TilePos, WorldFragment,
  WorldPos, WorldRect,
};
pub use culling::{CullingConfig, StreamCulled};
pub use material::{Material, Materials, PhysicsState, ids as material_ids};
pub use persistence::{PixelBodyRecord, WorldSave, WorldSaveResource};
pub use pixel::{Pixel, PixelFlags, PixelSurface};
pub use pixel_body::{
  DisplacementState, LastBlitTransform, NeedsColliderRegen, PendingPixelBody, Persistable,
  PixelBody, PixelBodyId, PixelBodyIdGenerator, PixelBodyLoader, SpawnPixelBody,
  SpawnPixelBodyFromImage, finalize_pending_pixel_bodies, generate_collider, update_pixel_bodies,
};
pub use primitives::{Chunk, Surface};
pub use render::{
  ChunkMaterial, Rgba, create_chunk_quad, create_palette_texture, create_pixel_texture,
  create_texture, materialize, rgb, spawn_static_chunk, upload_palette, upload_pixels,
  upload_surface,
};
pub use seeding::{ChunkSeeder, MaterialSeeder, NoiseSeeder, PersistenceSeeder};
pub use simulation::simulate_tick;
pub use text::{CpuFont, TextMask, TextStyle, draw_text, rasterize_text, stamp_text};
#[cfg(feature = "tracy")]
pub use tracy_init::init_tracy;
pub use world::control::{
  PersistenceComplete, PersistenceControl, PersistenceFuture, PersistenceHandle,
  RequestPersistence, SimulationState,
};
pub use world::plugin::{SeededChunks, StreamingCamera, UnloadingChunks};
pub use world::{PixelWorld, PixelWorldBundle, PixelWorldConfig, SpawnPixelWorld};

/// Configuration for chunk persistence.
#[derive(Clone, Debug)]
pub struct PersistenceConfig {
  /// Whether persistence is enabled.
  pub enabled: bool,
  /// Application name for default save directory.
  /// Used to create `~/.local/share/<app_name>/saves/` on Linux, etc.
  pub app_name: String,
  /// Explicit save file path. If None, uses default directory with
  /// "world.save".
  pub save_path: Option<PathBuf>,
  /// Name of save to load (e.g., "world" loads "world.save").
  /// If None, defaults to "world".
  pub load_save: Option<String>,
  /// World seed for procedural generation fallback.
  pub world_seed: u64,
}

impl Default for PersistenceConfig {
  fn default() -> Self {
    Self {
      enabled: true,
      app_name: persistence::DEFAULT_APP_NAME.to_string(),
      save_path: None,
      load_save: None,
      world_seed: 42,
    }
  }
}

impl PersistenceConfig {
  /// Creates a new persistence config with the given app name.
  pub fn new(app_name: impl Into<String>) -> Self {
    Self {
      app_name: app_name.into(),
      ..Default::default()
    }
  }

  /// Disables persistence.
  pub fn disabled() -> Self {
    Self {
      enabled: false,
      ..Default::default()
    }
  }

  /// Sets an explicit save file path.
  pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
    self.save_path = Some(path.into());
    self
  }

  /// Sets the world seed.
  pub fn with_seed(mut self, seed: u64) -> Self {
    self.world_seed = seed;
    self
  }

  /// Sets the save name to load.
  pub fn load(mut self, name: &str) -> Self {
    self.load_save = Some(name.to_string());
    self
  }

  /// Returns the effective save name.
  pub fn effective_save_name(&self) -> &str {
    self.load_save.as_deref().unwrap_or("world")
  }

  /// Returns the base directory for saves.
  pub fn save_dir(&self) -> PathBuf {
    self
      .save_path
      .as_ref()
      .and_then(|p| p.parent().map(|p| p.to_path_buf()))
      .unwrap_or_else(|| persistence::default_save_dir(&self.app_name))
  }

  /// Returns the effective save path for a given save name.
  pub fn save_path_for(&self, name: &str) -> PathBuf {
    self.save_dir().join(format!("{}.save", name))
  }

  /// Returns the effective save path.
  pub fn effective_path(&self) -> PathBuf {
    self
      .save_path
      .clone()
      .unwrap_or_else(|| self.save_path_for(self.effective_save_name()))
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
  pub culling: culling::CullingConfig,
}

impl PixelWorldPlugin {
  /// Creates a plugin with persistence enabled for the given app name.
  pub fn with_persistence(app_name: impl Into<String>) -> Self {
    Self {
      config: PixelWorldConfig::default(),
      persistence: PersistenceConfig::new(app_name),
      culling: culling::CullingConfig::default(),
    }
  }

  /// Sets the persistence configuration.
  pub fn persistence(mut self, config: PersistenceConfig) -> Self {
    self.persistence = config;
    self
  }

  /// Sets the culling configuration.
  pub fn culling(mut self, config: culling::CullingConfig) -> Self {
    self.culling = config;
    self
  }

  /// Sets the save name to load.
  pub fn load(mut self, name: &str) -> Self {
    self.persistence = self.persistence.load(name);
    self
  }
}

impl Plugin for PixelWorldPlugin {
  fn build(&self, app: &mut App) {
    // Embed the chunk shader and register material (rendering only)
    #[cfg(not(feature = "headless"))]
    {
      bevy::asset::embedded_asset!(app, "render/shaders/chunk.wgsl");
      app.add_plugins(Material2dPlugin::<ChunkMaterial>::default());
    }

    // Initialize Materials registry (users can override by inserting before plugin)
    app.init_resource::<Materials>();

    // Initialize pixel body ID generator
    app.init_resource::<pixel_body::PixelBodyIdGenerator>();

    // Store default config as resource for SpawnPixelWorld
    app.insert_resource(DefaultPixelWorldConfig(self.config.clone()));

    // Store persistence config
    app.insert_resource(DefaultPersistenceConfig(self.persistence.clone()));

    // Store culling config
    app.insert_resource(self.culling.clone());

    // Initialize persistence control with named save info
    let base_dir = self.persistence.save_dir();
    let current_save = self.persistence.effective_save_name().to_string();
    app.insert_resource(world::control::PersistenceControl::new(
      base_dir,
      current_save,
    ));

    // Initialize world save if persistence is enabled
    if self.persistence.enabled {
      let save_path = self.persistence.effective_path();
      match persistence::WorldSave::open_or_create(&save_path, self.persistence.world_seed) {
        Ok(save) => {
          info!("Opened world save at {:?}", save_path);
          app.insert_resource(persistence::WorldSaveResource::new(save));
        }
        Err(e) => {
          error!(
            "Failed to open world save at {:?}: {}. Persistence disabled.",
            save_path, e
          );
        }
      }
    }

    // Add world streaming systems
    app.add_plugins(world::plugin::PixelWorldStreamingPlugin);

    // Add visual debug plugin if feature is enabled
    #[cfg(feature = "visual_debug")]
    app.add_plugins(visual_debug::VisualDebugPlugin);
  }
}

/// Resource holding the default configuration for spawned pixel worlds.
#[derive(Resource)]
pub struct DefaultPixelWorldConfig(pub PixelWorldConfig);

/// Resource holding the default persistence configuration.
#[derive(Resource)]
pub struct DefaultPersistenceConfig(pub PersistenceConfig);
