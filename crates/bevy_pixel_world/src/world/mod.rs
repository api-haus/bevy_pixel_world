//! PixelWorld - unified chunk streaming and modification API.
//!
//! This module encapsulates all chunk management:
//! - Owns all chunk memory (no separate pool)
//! - Handles streaming window logic internally
//! - Provides world-coordinate pixel modification API
//! - Uses async background seeding with proper state tracking

mod body_loader;
mod bundle;
pub mod control;
pub(crate) mod persistence_systems;
pub mod plugin;
mod pool;
pub(crate) mod slot;
mod streaming;

use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;
pub use bundle::{PixelWorldBundle, SpawnPixelWorld};
use pool::ChunkPool;
pub(crate) use slot::{ChunkSlot, SlotIndex};
pub(crate) use streaming::{ChunkSaveData, StreamingDelta};
use streaming::{compute_position_changes, visible_positions};

use crate::coords::{ChunkPos, TilePos, WorldFragment, WorldPos, WorldRect};
use crate::debug_shim::{self, DebugGizmos};
use crate::pixel::Pixel;
use crate::primitives::Chunk;
#[cfg(not(feature = "headless"))]
use crate::render::ChunkMaterial;
use crate::scheduling::blitter::{Canvas, parallel_blit};
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

  /// Marks a chunk as needing GPU upload.
  pub fn mark_dirty(&mut self, pos: ChunkPos) {
    if let Some(idx) = self.pool.index_for(pos) {
      self.pool.get_mut(idx).dirty = true;
    }
  }

  /// Marks a world position as simulation-dirty.
  ///
  /// This expands the tile dirty rect so the CA simulation will process
  /// the pixel on the next tick. Use this when placing material that needs
  /// to participate in simulation (e.g., displaced water).
  pub fn mark_pixel_sim_dirty(&mut self, pos: WorldPos) {
    let (chunk_pos, local_pos) = pos.to_chunk_and_local();
    let Some(idx) = self.pool.index_for(chunk_pos) else {
      return;
    };
    let slot = self.pool.get_mut(idx);
    if !slot.is_seeded() {
      return;
    }
    slot
      .chunk
      .mark_pixel_dirty(local_pos.x as u32, local_pos.y as u32);
  }

  /// Returns an iterator over active chunk positions and their slot indices.
  pub(crate) fn active_chunks(&self) -> impl Iterator<Item = (ChunkPos, SlotIndex)> + '_ {
    self.pool.iter_active()
  }

  /// Collects mutable references to all seeded chunks for parallel access.
  ///
  /// Delegates to ChunkPool which encapsulates the unsafe pointer handling.
  pub(crate) fn collect_seeded_chunks(&mut self) -> HashMap<ChunkPos, &mut Chunk> {
    self.pool.collect_seeded_mut()
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
        eprintln!("Pool exhausted at {:?}", pos);
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
        eprintln!("Pool exhausted at {:?}", pos);
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

  // === Pixel access API ===

  /// Returns a reference to the pixel at the given world position.
  ///
  /// Returns None if the chunk is not loaded or not yet seeded.
  pub fn get_pixel(&self, pos: WorldPos) -> Option<&Pixel> {
    let (chunk_pos, local_pos) = pos.to_chunk_and_local();
    let idx = self.pool.index_for(chunk_pos)?;
    let slot = self.pool.get(idx);
    if !slot.is_seeded() {
      return None;
    }
    Some(&slot.chunk.pixels[(local_pos.x as u32, local_pos.y as u32)])
  }

  /// Returns a mutable reference to the pixel at the given world position.
  ///
  /// Returns None if the chunk is not loaded or not yet seeded.
  /// Does NOT mark the chunk as dirty - caller must do so.
  pub fn get_pixel_mut(&mut self, pos: WorldPos) -> Option<&mut Pixel> {
    let (chunk_pos, local_pos) = pos.to_chunk_and_local();
    let idx = self.pool.index_for(chunk_pos)?;
    let slot = self.pool.get_mut(idx);
    if !slot.is_seeded() {
      return None;
    }
    Some(&mut slot.chunk.pixels[(local_pos.x as u32, local_pos.y as u32)])
  }

  /// Swaps two pixels at the given world positions.
  ///
  /// Returns true if the swap was successful, false if either chunk
  /// is not loaded or not yet seeded.
  pub fn swap_pixels(&mut self, a: WorldPos, b: WorldPos) -> bool {
    let (chunk_a, local_a) = a.to_chunk_and_local();
    let (chunk_b, local_b) = b.to_chunk_and_local();

    // Get slot indices for both chunks
    let Some(idx_a) = self.pool.index_for(chunk_a) else {
      return false;
    };
    let Some(idx_b) = self.pool.index_for(chunk_b) else {
      return false;
    };

    // Check both are seeded
    if !self.pool.get(idx_a).is_seeded() || !self.pool.get(idx_b).is_seeded() {
      return false;
    }

    if chunk_a == chunk_b {
      // Same chunk - simple swap
      let slot = self.pool.get_mut(idx_a);
      let (la, lb) = (
        (local_a.x as u32, local_a.y as u32),
        (local_b.x as u32, local_b.y as u32),
      );
      let pixel_a = slot.chunk.pixels[la];
      let pixel_b = slot.chunk.pixels[lb];
      slot.chunk.pixels[la] = pixel_b;
      slot.chunk.pixels[lb] = pixel_a;
      slot.dirty = true;
      slot.modified = true;
      slot.persisted = false;
    } else {
      // Different chunks - need to swap across
      let (slot_a, slot_b) = self.pool.get_two_mut(idx_a, idx_b);

      let la = (local_a.x as u32, local_a.y as u32);
      let lb = (local_b.x as u32, local_b.y as u32);
      std::mem::swap(&mut slot_a.chunk.pixels[la], &mut slot_b.chunk.pixels[lb]);
      slot_a.dirty = true;
      slot_a.modified = true;
      slot_a.persisted = false;
      slot_b.dirty = true;
      slot_b.modified = true;
      slot_b.persisted = false;
    }

    true
  }

  /// Sets the pixel at the given world position.
  ///
  /// Returns true if the pixel was set, false if the chunk is not loaded
  /// or not yet seeded.
  ///
  /// The `debug_gizmos` parameter emits visual debug overlays when the
  /// `visual-debug` feature is enabled. Pass `()` when disabled.
  pub fn set_pixel(&mut self, pos: WorldPos, pixel: Pixel, debug_gizmos: DebugGizmos<'_>) -> bool {
    let (chunk_pos, local_pos) = pos.to_chunk_and_local();
    let Some(idx) = self.pool.index_for(chunk_pos) else {
      return false;
    };
    let slot = self.pool.get_mut(idx);
    if !slot.is_seeded() {
      return false;
    }
    slot.chunk.pixels[(local_pos.x as u32, local_pos.y as u32)] = pixel;
    let was_clean = !slot.dirty;
    slot.dirty = true;
    slot.modified = true;
    slot.persisted = false; // Needs saving again

    // Emit chunk gizmo if this is the first modification
    if was_clean {
      debug_shim::emit_chunk(debug_gizmos, chunk_pos);
    }

    true
  }

  /// Blits pixels using a shader-style callback.
  ///
  /// For each pixel in `rect`, calls `f(fragment)` where fragment contains
  /// world coordinates and normalized UV. If `f` returns Some(pixel), that
  /// pixel is written; if None, the pixel is unchanged.
  ///
  /// Uses parallel 2x2 checkerboard scheduling for thread-safe concurrent
  /// writes. Returns the list of chunk positions that were modified.
  ///
  /// The `debug_gizmos` parameter emits visual debug overlays when the
  /// `visual-debug` feature is enabled. Pass `()` when disabled.
  pub fn blit<F>(&mut self, rect: WorldRect, f: F, debug_gizmos: DebugGizmos<'_>) -> Vec<ChunkPos>
  where
    F: Fn(WorldFragment) -> Option<Pixel> + Sync,
  {
    let chunks = self.collect_seeded_chunks();
    let chunk_access = Canvas::new(chunks);
    let dirty_chunks = std::sync::Mutex::new(std::collections::HashSet::new());
    let dirty_tiles = std::sync::Mutex::new(std::collections::HashSet::<TilePos>::new());

    parallel_blit(&chunk_access, rect, f, &dirty_chunks, Some(&dirty_tiles));

    let dirty: Vec<_> = dirty_chunks
      .into_inner()
      .unwrap_or_default()
      .into_iter()
      .collect();
    let dirty_tile_list: Vec<_> = dirty_tiles
      .into_inner()
      .unwrap_or_default()
      .into_iter()
      .collect();

    // Mark affected chunks as dirty and needing save
    for &pos in &dirty {
      if let Some(idx) = self.pool.index_for(pos) {
        let slot = self.pool.get_mut(idx);
        slot.dirty = true;
        slot.modified = true;
        slot.persisted = false;
      }
    }

    // Emit debug gizmos
    debug_shim::emit_blit_rect(debug_gizmos, rect);
    for &pos in &dirty {
      debug_shim::emit_chunk(debug_gizmos, pos);
    }
    for &tile in &dirty_tile_list {
      debug_shim::emit_tile(debug_gizmos, tile);
    }

    dirty
  }
}
