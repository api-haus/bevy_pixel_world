//! PixelWorld - unified chunk streaming and modification API.
//!
//! This module encapsulates all chunk management:
//! - Owns all chunk memory (no separate pool)
//! - Handles streaming window logic internally
//! - Provides world-coordinate pixel modification API
//! - Uses async background seeding with proper state tracking

mod bundle;
pub mod plugin;
pub(crate) mod slot;
mod streaming;

use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;

use crate::coords::{ChunkPos, TilePos, WorldFragment, WorldPos, WorldRect, POOL_SIZE};
use crate::debug_shim::{self, DebugGizmos};
use crate::pixel::Pixel;
use crate::primitives::Chunk;
#[cfg(not(feature = "headless"))]
use crate::render::ChunkMaterial;
use crate::scheduling::blitter::{parallel_blit, Canvas};
use crate::seeding::ChunkSeeder;

pub use bundle::{PixelWorldBundle, SpawnPixelWorld};
pub(crate) use slot::{ChunkSlot, SlotIndex};
pub(crate) use streaming::{ChunkSaveData, StreamingDelta};
use streaming::visible_positions;

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
  /// Fixed array of chunk slots (pre-allocated memory).
  slots: Vec<ChunkSlot>,
  /// Maps active chunk positions to slot indices.
  active: HashMap<ChunkPos, SlotIndex>,
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
    let slots = (0..POOL_SIZE).map(|_| ChunkSlot::new()).collect();

    Self {
      center: ChunkPos::new(0, 0),
      slots,
      active: HashMap::new(),
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

  /// Acquires a free slot from the pool.
  ///
  /// Returns None if all slots are in use.
  fn acquire_slot(&mut self) -> Option<SlotIndex> {
    for (i, slot) in self.slots.iter_mut().enumerate() {
      if slot.is_free() {
        return Some(SlotIndex(i));
      }
    }
    None
  }

  /// Gets a reference to a slot by index.
  pub(crate) fn slot(&self, index: SlotIndex) -> &ChunkSlot {
    &self.slots[index.0]
  }

  /// Gets a mutable reference to a slot by index.
  pub(crate) fn slot_mut(&mut self, index: SlotIndex) -> &mut ChunkSlot {
    &mut self.slots[index.0]
  }

  /// Gets the slot index for an active chunk position.
  pub(crate) fn get_slot_index(&self, pos: ChunkPos) -> Option<SlotIndex> {
    self.active.get(&pos).copied()
  }

  /// Returns a mutable reference to chunk data at the given position.
  pub fn get_chunk_mut(&mut self, pos: ChunkPos) -> Option<&mut Chunk> {
    self
      .active
      .get(&pos)
      .map(|&idx| &mut self.slots[idx.0].chunk)
  }

  /// Marks a chunk as needing GPU upload.
  pub fn mark_dirty(&mut self, pos: ChunkPos) {
    if let Some(&idx) = self.active.get(&pos) {
      self.slots[idx.0].dirty = true;
    }
  }

  /// Returns an iterator over active chunk positions and their slot indices.
  pub(crate) fn active_chunks(&self) -> impl Iterator<Item = (ChunkPos, SlotIndex)> + '_ {
    self.active.iter().map(|(&pos, &idx)| (pos, idx))
  }

  /// Collects mutable references to all seeded chunks for parallel access.
  ///
  /// # Safety
  /// This method uses raw pointers to work around the borrow checker.
  /// It is safe because:
  /// - Each slot appears at most once in `self.active` (unique SlotIndex values)
  /// - Slots are stored in a Vec with distinct indices
  /// - The resulting mutable references are non-overlapping
  pub(crate) fn collect_seeded_chunks(&mut self) -> HashMap<ChunkPos, &mut Chunk> {
    let seeded_positions: Vec<_> = self
      .active
      .iter()
      .filter_map(|(&pos, &idx)| {
        if self.slots[idx.0].seeded {
          Some((pos, idx))
        } else {
          None
        }
      })
      .collect();

    let mut chunks: HashMap<ChunkPos, &mut Chunk> = HashMap::new();
    for (pos, idx) in seeded_positions {
      // SAFETY: seeded_positions contains unique SlotIndex values.
      let chunk = &mut self.slots[idx.0].chunk;
      let chunk_ptr = chunk as *mut Chunk;
      chunks.insert(pos, unsafe { &mut *chunk_ptr });
    }
    chunks
  }

  /// Returns the number of active chunks.
  pub fn active_count(&self) -> usize {
    self.active.len()
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
      if let Some(idx) = self.acquire_slot() {
        let slot = &mut self.slots[idx.0];
        slot.lifecycle = slot::ChunkLifecycle::Seeding;
        slot.pos = Some(pos);
        slot.chunk.set_pos(pos);
        slot.seeded = false;
        slot.dirty = false;
        slot.modified = false;
        slot.persisted = false;
        self.active.insert(pos, idx);
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
      return StreamingDelta {
        to_despawn: vec![],
        to_spawn: vec![],
        to_save: vec![],
      };
    }

    // Compute old and new visible sets
    let old_set: std::collections::HashSet<_> = self.visible_positions().collect();
    self.center = new_center;
    let new_set: std::collections::HashSet<_> = self.visible_positions().collect();

    // Find chunks to release (in old but not new)
    let mut to_despawn = Vec::new();
    let mut to_save = Vec::new();
    for pos in old_set.difference(&new_set) {
      if let Some(idx) = self.active.remove(pos) {
        let slot = &mut self.slots[idx.0];
        let entity = slot.entity;

        // Clone pixel data for saving before release
        if slot.needs_save() {
          to_save.push(ChunkSaveData {
            pos: *pos,
            pixels: slot.chunk.pixels.as_bytes().to_vec(),
          });
        }

        slot.release();
        if let Some(entity) = entity {
          to_despawn.push((*pos, entity));
        }
      }
    }

    // Find chunks to spawn (in new but not old)
    let mut to_spawn = Vec::new();
    for pos in new_set.difference(&old_set) {
      if let Some(idx) = self.acquire_slot() {
        let slot = &mut self.slots[idx.0];
        slot.lifecycle = slot::ChunkLifecycle::Seeding;
        slot.pos = Some(*pos);
        slot.chunk.set_pos(*pos);
        slot.seeded = false;
        slot.dirty = false;
        slot.modified = false;
        slot.persisted = false;
        self.active.insert(*pos, idx);
        to_spawn.push((*pos, idx));
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

  /// Registers entity and render resources for a slot.
  #[cfg(not(feature = "headless"))]
  pub(crate) fn register_slot_entity(
    &mut self,
    index: SlotIndex,
    entity: Entity,
    texture: Handle<Image>,
    material: Handle<ChunkMaterial>,
  ) {
    let slot = &mut self.slots[index.0];
    slot.entity = Some(entity);
    slot.texture = Some(texture);
    slot.material = Some(material);
  }

  /// Registers entity for a slot (headless mode - no render resources).
  #[cfg(feature = "headless")]
  pub(crate) fn register_slot_entity_headless(&mut self, index: SlotIndex, entity: Entity) {
    let slot = &mut self.slots[index.0];
    slot.entity = Some(entity);
  }

  // === Pixel access API ===

  /// Returns a reference to the pixel at the given world position.
  ///
  /// Returns None if the chunk is not loaded or not yet seeded.
  pub fn get_pixel(&self, pos: WorldPos) -> Option<&Pixel> {
    let (chunk_pos, local_pos) = pos.to_chunk_and_local();
    let idx = self.active.get(&chunk_pos)?;
    let slot = &self.slots[idx.0];
    if !slot.seeded {
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
    let idx = self.active.get(&chunk_pos)?;
    let slot = &mut self.slots[idx.0];
    if !slot.seeded {
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
    let Some(&idx_a) = self.active.get(&chunk_a) else {
      return false;
    };
    let Some(&idx_b) = self.active.get(&chunk_b) else {
      return false;
    };

    // Check both are seeded
    if !self.slots[idx_a.0].seeded || !self.slots[idx_b.0].seeded {
      return false;
    }

    if chunk_a == chunk_b {
      // Same chunk - simple swap
      let slot = &mut self.slots[idx_a.0];
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
      // SAFETY: idx_a != idx_b since chunk_a != chunk_b and active map is 1:1
      let (slot_a, slot_b) = if idx_a.0 < idx_b.0 {
        let (left, right) = self.slots.split_at_mut(idx_b.0);
        (&mut left[idx_a.0], &mut right[0])
      } else {
        let (left, right) = self.slots.split_at_mut(idx_a.0);
        (&mut right[0], &mut left[idx_b.0])
      };

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
    let Some(&idx) = self.active.get(&chunk_pos) else {
      return false;
    };
    let slot = &mut self.slots[idx.0];
    if !slot.seeded {
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
      if let Some(&idx) = self.active.get(&pos) {
        self.slots[idx.0].dirty = true;
        self.slots[idx.0].modified = true;
        self.slots[idx.0].persisted = false;
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
