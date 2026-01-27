//! PixelWorld - unified chunk streaming and modification API.
//!
//! This module encapsulates all chunk management:
//! - Owns all chunk memory (no separate pool)
//! - Handles streaming window logic internally
//! - Provides world-coordinate pixel modification API
//! - Uses async background seeding with proper state tracking
//!
//! Sub-modules split `PixelWorld` methods by responsibility:
//! - [`pixel_access`] — world-coordinate pixel read/write/swap
//! - [`blit`] — parallel blit orchestration

mod blit;
mod body_loader;
mod bundle;
pub mod control;
pub(crate) mod persistence_systems;
mod pixel_access;
pub mod plugin;
mod pool;
pub(crate) mod slot;
pub(crate) mod streaming;
pub(crate) mod systems;

use std::sync::Arc;

use bevy::prelude::*;
pub use bundle::{PixelWorldBundle, SpawnPixelWorld};
use pool::ChunkPool;
pub(crate) use slot::{ChunkSlot, SlotIndex};
pub(crate) use streaming::{ChunkSaveData, StreamingDelta};
use streaming::{compute_position_changes, visible_positions};

use crate::coords::{ChunkPos, WorldRect};
use crate::primitives::Chunk;
#[cfg(not(feature = "headless"))]
use crate::render::ChunkMaterial;
use crate::seeding::ChunkSeeder;

/// Configuration for pixel world simulation behavior.
#[derive(Clone, Debug)]
pub struct PixelWorldConfig {
  /// Jitter factor for tile grid offset (0.0 = no jitter, 1.0 = full tile
  /// jitter). Higher values reduce tile boundary artifacts but may slightly
  /// increase processing.
  pub jitter_factor: f32,
}

impl Default for PixelWorldConfig {
  fn default() -> Self {
    Self { jitter_factor: 0.0 }
  }
}

/// Unified pixel world component.
///
/// Owns all chunk memory and handles streaming, seeding, and modification.
/// Spawn this as a component on an entity to create a pixel world.
#[derive(Component)]
pub struct PixelWorld {
  /// Current center of the streaming window in chunk coordinates.
  center: ChunkPos,
  /// Pool of chunk slots with position-to-index mapping.
  pool: ChunkPool,
  /// Chunk seeder for generating initial data.
  seeder: Arc<dyn ChunkSeeder + Send + Sync>,
  /// Shared mesh for all chunk entities.
  mesh: Handle<Mesh>,
  /// Simulation seed for deterministic randomness.
  seed: u64,
  /// Current simulation tick.
  tick: u64,
  /// World configuration settings.
  config: PixelWorldConfig,
  /// Optional viewport bounds for simulation culling.
  /// When set, only tiles overlapping these bounds are simulated.
  simulation_bounds: Option<WorldRect>,
  /// Margin in pixels added to simulation bounds (default: 64, ~2 tiles).
  simulation_margin: i64,
}

impl PixelWorld {
  /// Creates a new pixel world with the given seeder and mesh.
  pub fn new(seeder: Arc<dyn ChunkSeeder + Send + Sync>, mesh: Handle<Mesh>) -> Self {
    Self::with_config(seeder, mesh, PixelWorldConfig::default())
  }

  /// Creates a new pixel world with a specific simulation seed.
  pub fn with_seed(
    seeder: Arc<dyn ChunkSeeder + Send + Sync>,
    mesh: Handle<Mesh>,
    seed: u64,
  ) -> Self {
    Self::with_config_and_seed(seeder, mesh, PixelWorldConfig::default(), seed)
  }

  /// Creates a new pixel world with custom configuration.
  pub fn with_config(
    seeder: Arc<dyn ChunkSeeder + Send + Sync>,
    mesh: Handle<Mesh>,
    config: PixelWorldConfig,
  ) -> Self {
    Self::with_config_and_seed(seeder, mesh, config, 0)
  }

  /// Creates a new pixel world with custom configuration and seed.
  pub fn with_config_and_seed(
    seeder: Arc<dyn ChunkSeeder + Send + Sync>,
    mesh: Handle<Mesh>,
    config: PixelWorldConfig,
    seed: u64,
  ) -> Self {
    Self {
      center: ChunkPos::new(0, 0),
      pool: ChunkPool::new(),
      seeder,
      mesh,
      seed,
      tick: 0,
      config,
      simulation_bounds: None,
      simulation_margin: 64,
    }
  }

  /// Returns the current center chunk position.
  pub fn center(&self) -> ChunkPos {
    self.center
  }

  /// Returns the simulation seed.
  pub fn seed(&self) -> u64 {
    self.seed
  }

  /// Returns the current simulation tick.
  pub fn tick(&self) -> u64 {
    self.tick
  }

  /// Increments the simulation tick counter.
  pub fn increment_tick(&mut self) {
    self.tick = self.tick.wrapping_add(1);
  }

  /// Returns the world configuration.
  pub fn config(&self) -> &PixelWorldConfig {
    &self.config
  }

  /// Returns a mutable reference to the world configuration.
  pub fn config_mut(&mut self) -> &mut PixelWorldConfig {
    &mut self.config
  }

  /// Sets the simulation bounds for viewport culling.
  ///
  /// When set, only tiles overlapping these bounds (plus margin) are simulated.
  /// Pass `None` to simulate all tiles in the streaming window.
  pub fn set_simulation_bounds(&mut self, bounds: Option<WorldRect>) {
    self.simulation_bounds = bounds;
  }

  /// Returns the simulation bounds expanded by the margin.
  ///
  /// Returns `None` if no bounds are set (full streaming window simulation).
  pub fn simulation_bounds(&self) -> Option<WorldRect> {
    self.simulation_bounds.map(|rect| {
      WorldRect::new(
        rect.x - self.simulation_margin,
        rect.y - self.simulation_margin,
        rect.width + (self.simulation_margin * 2) as u32,
        rect.height + (self.simulation_margin * 2) as u32,
      )
    })
  }

  /// Returns the shared mesh handle.
  pub fn mesh(&self) -> &Handle<Mesh> {
    &self.mesh
  }

  /// Returns the seeder.
  pub fn seeder(&self) -> &Arc<dyn ChunkSeeder + Send + Sync> {
    &self.seeder
  }

  /// Returns iterator over visible chunk positions for the current center.
  pub fn visible_positions(&self) -> impl Iterator<Item = ChunkPos> {
    visible_positions(self.center)
  }

  /// Gets a reference to a slot by index.
  pub(crate) fn slot(&self, index: SlotIndex) -> &ChunkSlot {
    self.pool.get(index)
  }

  /// Gets a mutable reference to a slot by index.
  pub(crate) fn slot_mut(&mut self, index: SlotIndex) -> &mut ChunkSlot {
    self.pool.get_mut(index)
  }

  /// Gets the slot index for an active chunk position.
  pub(crate) fn get_slot_index(&self, pos: ChunkPos) -> Option<SlotIndex> {
    self.pool.index_for(pos)
  }

  /// Returns a mutable reference to chunk data at the given position.
  pub fn get_chunk_mut(&mut self, pos: ChunkPos) -> Option<&mut Chunk> {
    self
      .pool
      .index_for(pos)
      .map(|idx| &mut self.pool.get_mut(idx).chunk)
  }

  /// Returns an iterator over active chunk positions and their slot indices.
  pub(crate) fn active_chunks(&self) -> impl Iterator<Item = (ChunkPos, SlotIndex)> + '_ {
    self.pool.iter_active()
  }

  /// Returns the number of active chunks.
  pub fn active_count(&self) -> usize {
    self.pool.active_count()
  }

  // === Streaming logic ===

  /// Initializes the world at a given center position.
  ///
  /// Used for initial spawn when there are no active chunks yet.
  /// Returns all visible positions as chunks to spawn.
  pub(crate) fn initialize_at(&mut self, center: ChunkPos) -> StreamingDelta {
    self.center = center;

    // Collect positions first to avoid borrow issues
    let positions: Vec<_> = visible_positions(center).collect();

    let mut to_spawn = Vec::new();
    for pos in positions {
      if let Some(idx) = self.pool.acquire() {
        self.pool.get_mut(idx).initialize(pos);
        self.pool.activate(pos, idx);
        to_spawn.push((pos, idx));
      } else {
        warn!("Pool exhausted at {:?}", pos);
      }
    }

    StreamingDelta {
      to_despawn: vec![],
      to_spawn,
      to_save: vec![],
    }
  }

  /// Updates the streaming window center, returning positions to despawn and
  /// spawn.
  ///
  /// This handles:
  /// - Computing which chunks leave/enter the window
  /// - Releasing slots for departing chunks
  /// - Acquiring slots for arriving chunks
  /// - Marking new chunks as unseeded
  pub(crate) fn update_center(&mut self, new_center: ChunkPos) -> StreamingDelta {
    if new_center == self.center {
      return StreamingDelta::empty();
    }

    let (leaving, entering) = compute_position_changes(self.center, new_center);
    self.center = new_center;

    // Release chunks that are leaving the window
    let mut to_despawn = Vec::new();
    let mut to_save = Vec::new();
    for pos in leaving {
      if let Some(idx) = self.pool.deactivate(&pos) {
        let slot = self.pool.get_mut(idx);
        let entity = slot.entity;

        // Clone pixel data for saving before release
        if slot.needs_save() {
          to_save.push(ChunkSaveData {
            pos,
            pixels: slot.chunk.pixels.as_bytes().to_vec(),
          });
        }

        slot.release();
        if let Some(entity) = entity {
          to_despawn.push((pos, entity));
        }
      }
    }

    // Acquire slots for chunks entering the window
    let mut to_spawn = Vec::new();
    for pos in entering {
      if let Some(idx) = self.pool.acquire() {
        self.pool.get_mut(idx).initialize(pos);
        self.pool.activate(pos, idx);
        to_spawn.push((pos, idx));
      } else {
        warn!("Pool exhausted at {:?}", pos);
      }
    }

    StreamingDelta {
      to_despawn,
      to_spawn,
      to_save,
    }
  }

  /// Registers entity and optional render resources for a slot.
  pub(crate) fn register_slot_entity(
    &mut self,
    index: SlotIndex,
    entity: Entity,
    #[cfg(not(feature = "headless"))] texture: Handle<Image>,
    #[cfg(not(feature = "headless"))] material: Handle<ChunkMaterial>,
  ) {
    let slot = self.pool.get_mut(index);
    slot.entity = Some(entity);
    #[cfg(not(feature = "headless"))]
    {
      slot.texture = Some(texture);
      slot.material = Some(material);
    }
  }
}
