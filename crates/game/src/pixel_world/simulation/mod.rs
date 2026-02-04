//! Cellular automata simulation.
//!
//! Implements falling sand physics, burning propagation, and heat diffusion
//! using checkerboard scheduling for parallel processing.
//!
//! # Simulation Passes
//!
//! Three independent simulation systems run at different tick rates:
//!
//! | System | Tick Rate | Scheduling | Description |
//! |--------|-----------|------------|-------------|
//! | Physics | every tick | Checkerboard | Pixel swaps, falling sand |
//! | Burning | every Nth tick | Checkerboard | Fire spread, ash transformation |
//! | Heat | every Mth tick | Sequential | Heat diffusion on downsampled grid |

pub(crate) mod burning;
mod config;
pub(crate) mod hash;
mod heat;
pub(crate) mod physics;

use std::collections::HashSet;
use std::sync::Mutex;

use burning::BurningContext;
pub use config::SimulationConfig;
use hash::hash21uu64;
pub use heat::HeatConfig;

use crate::pixel_world::coords::{
  ChunkPos, Phase, TILE_SIZE, TILES_PER_CHUNK, TilePos, WINDOW_HEIGHT, WINDOW_WIDTH, WorldRect,
};
use crate::pixel_world::debug_shim::DebugGizmos;
use crate::pixel_world::diagnostics::profile;
use crate::pixel_world::material::Materials;
use crate::pixel_world::scheduling::blitter::{Canvas, parallel_burning, parallel_simulate};
use crate::pixel_world::world::PixelWorld;

/// Context passed to simulation rules for deterministic randomness.
#[derive(Clone, Copy)]
pub struct SimContext {
  /// Level seed for reproducible randomness.
  pub seed: u64,
  /// Current simulation tick.
  pub tick: u64,
  /// Tile grid jitter X offset (0 to TILE_SIZE-1).
  pub jitter_x: i64,
  /// Tile grid jitter Y offset (0 to TILE_SIZE-1).
  pub jitter_y: i64,
}

/// Runs one simulation tick on the world using parallel tile processing.
///
/// Orchestrates three simulation passes at different tick rates:
/// - Physics (every tick): Pixel swaps using dirty rects
/// - Burning (every Nth tick): Fire spread using dirty rects
/// - Heat (every Mth tick): Heat diffusion on downsampled grid
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all, fields(tick = world.tick())))]
pub fn simulate_tick(
  world: &mut PixelWorld,
  materials: &Materials,
  debug_gizmos: DebugGizmos<'_>,
  sim_config: &SimulationConfig,
  heat_config: &HeatConfig,
) {
  let _span = profile("simulate_tick");

  // Get context before borrowing chunks
  let center = world.center();
  let tick = world.tick();

  // Generate per-tick jitter for tile grid offset
  let max_jitter = (TILE_SIZE as f32 * world.config().jitter_factor) as u64;
  let (jitter_x, jitter_y) = if max_jitter > 0 {
    (
      (hash21uu64(tick, 0) % max_jitter) as i64,
      (hash21uu64(tick, 1) % max_jitter) as i64,
    )
  } else {
    (0, 0)
  };

  let ctx = SimContext {
    seed: world.seed(),
    tick,
    jitter_x,
    jitter_y,
  };
  let simulation_bounds = world.simulation_bounds();
  let tiles_by_phase = {
    let _span = profile("collect_tiles");
    collect_tiles_by_phase(center, simulation_bounds)
  };

  // Increment tick for next frame
  world.increment_tick();

  // Collect seeded chunks for parallel access
  let chunks_map = {
    let _span = profile("collect_chunks");
    world.collect_seeded_chunks()
  };
  if chunks_map.is_empty() {
    return;
  }

  let chunk_access = Canvas::new(chunks_map);
  let dirty = Mutex::new(HashSet::new());

  // === Pass 1: Physics simulation (every tick, ~60 TPS) ===
  {
    let _span = profile("physics");
    parallel_simulate(
      &chunk_access,
      tiles_by_phase.clone(),
      |pos, chunks| physics::compute_swap(pos, chunks, materials, ctx),
      &dirty,
      debug_gizmos,
      ctx.tick,
      (jitter_x, jitter_y),
    );
  }

  // Compute tick intervals from TPS ratios
  let burning_interval = (sim_config.physics_tps / sim_config.burning_tps).round() as u64;
  let heat_interval = (sim_config.physics_tps / sim_config.heat_tps).round() as u64;

  // === Pass 2: Burning propagation (every Nth tick, ~20 TPS) ===
  if tick.is_multiple_of(burning_interval) {
    let _span = profile("burning");
    let burning_ctx = BurningContext {
      materials,
      ctx,
      // Convert tick-rate-independent config to per-tick probabilities
      spread_chance: heat_config.spread_chance_per_tick(sim_config.burning_tps),
      ash_chance: heat_config.ash_chance_per_tick(sim_config.burning_tps),
    };
    parallel_burning(
      &chunk_access,
      tiles_by_phase,
      &burning_ctx,
      &dirty,
      (jitter_x, jitter_y),
    );
  }

  // === Pass 3: Heat propagation (every Mth tick) ===
  // Operates on downsampled heat grid, no checkerboard needed
  let chunk_positions: Vec<ChunkPos> = chunk_access.positions().collect();
  if tick.is_multiple_of(heat_interval) {
    let _span = profile("heat");
    heat::propagate_heat(
      &chunk_access,
      &chunk_positions,
      materials,
      heat_config,
      debug_gizmos,
    );
    heat::ignite_from_heat(&chunk_access, &chunk_positions, materials);
  }

  // Drop canvas before using world again
  drop(chunk_access);

  // Mark dirty chunks for GPU upload
  for pos in dirty.into_inner().unwrap() {
    world.mark_dirty(pos);
  }
}

/// Collects tiles grouped by phase for the current visible region.
///
/// When `bounds` is `Some`, only tiles overlapping the bounds are collected.
/// The bounds should already include any desired margin.
#[cfg_attr(
  feature = "tracy",
  tracing::instrument(skip_all, name = "collect_tiles")
)]
fn collect_tiles_by_phase(center: ChunkPos, bounds: Option<WorldRect>) -> [Vec<TilePos>; 4] {
  let mut phases: [Vec<TilePos>; 4] = [vec![], vec![], vec![], vec![]];

  let hw = WINDOW_WIDTH as i32 / 2;
  let hh = WINDOW_HEIGHT as i32 / 2;

  // Compute streaming window tile bounds
  let tiles_per_chunk = TILES_PER_CHUNK as i64;
  let window_min_tx = (center.x - hw) as i64 * tiles_per_chunk;
  let window_max_tx = (center.x + hw) as i64 * tiles_per_chunk;
  let window_min_ty = (center.y - hh) as i64 * tiles_per_chunk;
  let window_max_ty = (center.y + hh) as i64 * tiles_per_chunk;

  if let Some(rect) = bounds {
    // Collect only tiles that overlap bounds AND are in streaming window
    for tile in rect.to_tile_range() {
      // Check if tile is within streaming window
      if tile.x >= window_min_tx
        && tile.x < window_max_tx
        && tile.y >= window_min_ty
        && tile.y < window_max_ty
      {
        let phase = Phase::from_tile(tile);
        phases[phase.index()].push(tile);
      }
    }
  } else {
    // No bounds - collect all tiles in streaming window
    for tile_y in window_min_ty..window_max_ty {
      for tile_x in window_min_tx..window_max_tx {
        let tile = TilePos::new(tile_x, tile_y);
        let phase = Phase::from_tile(tile);
        phases[phase.index()].push(tile);
      }
    }
  }

  phases
}
