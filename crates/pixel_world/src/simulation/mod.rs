//! Cellular automata simulation.
//!
//! Implements falling sand physics using checkerboard scheduling.

pub mod hash;
pub(crate) mod rules;

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use hash::hash21uu64;

use crate::coords::{ChunkPos, TilePos, CHUNK_SIZE, TILES_PER_CHUNK, TILE_SIZE};
use crate::debug_shim::DebugGizmos;
use crate::material::Materials;
use crate::parallel::blitter::{parallel_simulate, ChunkAccess};
use crate::primitives::Chunk;
use crate::world::PixelWorld;

/// Simulation phase for checkerboard scheduling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
  A, // (0, 0)
  B, // (1, 0)
  C, // (0, 1)
  D, // (1, 1)
}

impl Phase {
  /// Returns the phase for a tile at the given position.
  fn from_tile(tile: TilePos) -> Phase {
    let px = tile.x.rem_euclid(2);
    let py = tile.y.rem_euclid(2);
    match (px, py) {
      (0, 0) => Phase::A,
      (1, 0) => Phase::B,
      (0, 1) => Phase::C,
      (1, 1) => Phase::D,
      _ => unreachable!(),
    }
  }
}

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
/// Processes all four phases sequentially. Each phase processes all tiles
/// of that phase in parallel, which are never adjacent, ensuring thread-safe
/// access.
pub fn simulate_tick(world: &mut PixelWorld, materials: &Materials, debug_gizmos: DebugGizmos<'_>) {
  // Get context before borrowing chunks
  let center = world.center();
  let tick = world.tick();

  // Generate per-tick jitter for tile grid offset
  // TODO: Dirty rects stability still needs improvement with jitter enabled.
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
  let tiles_by_phase = collect_tiles_by_phase(center);

  // Increment tick for next frame
  world.increment_tick();

  // Extract mutable chunk references for parallel access
  let chunks_map = extract_chunks_for_simulation(world);
  if chunks_map.is_empty() {
    return;
  }

  let chunk_access = ChunkAccess::new(chunks_map);
  let dirty = Mutex::new(HashSet::new());

  parallel_simulate(
    &chunk_access,
    tiles_by_phase,
    |pos, chunks| rules::compute_swap(pos, chunks, materials, ctx),
    &dirty,
    debug_gizmos,
    ctx.tick,
    (jitter_x, jitter_y),
  );

  // Mark dirty chunks for GPU upload
  for pos in dirty.into_inner().unwrap() {
    world.mark_dirty(pos);
  }
}

/// Extracts mutable chunk references for parallel simulation.
fn extract_chunks_for_simulation(world: &mut PixelWorld) -> HashMap<ChunkPos, &mut Chunk> {
  let mut chunks: HashMap<ChunkPos, &mut Chunk> = HashMap::new();

  // Collect seeded chunk positions first
  let seeded_positions: Vec<_> = world
    .active_chunks()
    .filter_map(|(pos, idx)| {
      if world.slot(idx).seeded {
        Some((pos, idx))
      } else {
        None
      }
    })
    .collect();

  for (pos, idx) in seeded_positions {
    // SAFETY: Each slot index is unique in active_chunks.
    let chunk = &mut world.slot_mut(idx).chunk;
    let chunk_ptr = chunk as *mut Chunk;
    chunks.insert(pos, unsafe { &mut *chunk_ptr });
  }

  chunks
}

/// Collects tiles grouped by phase for the current visible region.
fn collect_tiles_by_phase(center: ChunkPos) -> [Vec<TilePos>; 4] {
  let mut phases: [Vec<TilePos>; 4] = [vec![], vec![], vec![], vec![]];

  let hw = 3i32; // WINDOW_WIDTH / 2
  let hh = 2i32; // WINDOW_HEIGHT / 2
  let tiles_per_chunk = TILES_PER_CHUNK as i64;
  let tile_size = TILE_SIZE as i64;

  for cy in (center.y - hh)..(center.y + hh) {
    for cx in (center.x - hw)..(center.x + hw) {
      let chunk_origin_x = cx as i64 * CHUNK_SIZE as i64;
      let chunk_origin_y = cy as i64 * CHUNK_SIZE as i64;

      for ty in 0..tiles_per_chunk {
        for tx in 0..tiles_per_chunk {
          let tile_world_x = chunk_origin_x + tx * tile_size;
          let tile_world_y = chunk_origin_y + ty * tile_size;
          let tile = TilePos::new(tile_world_x / tile_size, tile_world_y / tile_size);

          let phase = Phase::from_tile(tile);
          let idx = match phase {
            Phase::A => 0,
            Phase::B => 1,
            Phase::C => 2,
            Phase::D => 3,
          };
          phases[idx].push(tile);
        }
      }
    }
  }

  phases
}
