//! Heat layer propagation.
//!
//! The heat layer is a downsampled grid (1/4 resolution) per chunk. Each cell
//! accumulates heat from burning pixels and material base temperatures, then
//! diffuses to neighbors with a cooling factor.

use crate::pixel_world::coords::ChunkPos;
use crate::pixel_world::debug_shim::{DebugGizmos, emit_heat_dirty_tile};
use crate::pixel_world::material::Materials;
use crate::pixel_world::pixel::PixelFlags;
use crate::pixel_world::primitives::{Chunk, HEAT_CELL_SIZE, HEAT_CELLS_PER_TILE, HEAT_GRID_SIZE};
use crate::pixel_world::scheduling::blitter::Canvas;

/// Configuration for heat simulation.
///
/// Rate/duration parameters are tick-rate independent - they express behavior
/// in real-world time units (seconds) and are converted to per-tick
/// probabilities at runtime using the burning TPS from SimulationConfig.
#[derive(bevy::prelude::Resource)]
pub struct HeatConfig {
  /// Multiplier applied during diffusion (default 0.95).
  pub cooling_factor: f32,
  /// Heat emitted per burning pixel into its heat cell (default 50).
  pub burning_heat: u8,
  /// Fire spread rate: expected ignitions per second per burning pixel.
  /// Spread attempts are made to each cardinal neighbor independently.
  /// (default 2.0 = ~2 neighbors ignite per second)
  pub spread_rate: f32,
  /// Average time a burning pixel takes to turn to ash (seconds).
  /// This affects the per-tick probability of burn effects triggering.
  /// (default 5.0 = ~5 seconds average burn duration)
  pub burn_duration_secs: f32,
}

impl Default for HeatConfig {
  fn default() -> Self {
    Self {
      cooling_factor: 0.95,
      burning_heat: 50,
      spread_rate: 2.0,
      burn_duration_secs: 5.0,
    }
  }
}

impl HeatConfig {
  /// Converts spread_rate to per-tick probability for a single neighbor.
  ///
  /// Given N cardinal neighbors, each gets: spread_rate / (N * burning_tps)
  pub fn spread_chance_per_tick(&self, burning_tps: f32) -> f32 {
    const NUM_NEIGHBORS: f32 = 4.0; // Cardinal directions
    (self.spread_rate / (NUM_NEIGHBORS * burning_tps)).min(1.0)
  }

  /// Converts burn_duration_secs to per-tick probability of ash transformation.
  ///
  /// Uses Poisson process: p = 1 / (duration * tps)
  pub fn ash_chance_per_tick(&self, burning_tps: f32) -> f32 {
    (1.0 / (self.burn_duration_secs * burning_tps)).min(1.0)
  }
}

/// Accumulates heat from pixel sources within a single heat cell's 4x4 region.
/// Returns (source_heat, solid_count) where solid_count is the number of
/// non-void pixels.
fn accumulate_cell_heat_sources(
  chunk: &Chunk,
  hx: u32,
  hy: u32,
  materials: &Materials,
  burning_heat: u8,
) -> (u32, u32) {
  let px_base_x = hx * HEAT_CELL_SIZE;
  let px_base_y = hy * HEAT_CELL_SIZE;
  let mut source: u32 = 0;
  let mut solid_count: u32 = 0;

  for dy in 0..HEAT_CELL_SIZE {
    for dx in 0..HEAT_CELL_SIZE {
      let pixel = chunk.pixels[(px_base_x + dx, px_base_y + dy)];
      if pixel.is_void() {
        continue;
      }
      solid_count += 1;
      let mat = materials.get(pixel.material);
      source += mat.base_temperature as u32;
      if pixel.flags.contains(PixelFlags::BURNING) {
        source += burning_heat as u32;
      }
    }
  }

  (source, solid_count)
}

/// Cardinal offsets for heat neighbor sampling: (dx, dy).
const HEAT_CARDINAL: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

/// Samples heat from cardinal neighbors, handling both interior and cross-chunk
/// boundaries. Returns `(neighbor_sum, neighbor_count)`.
fn sample_heat_neighbors(
  hx: u32,
  hy: u32,
  chunk: &Chunk,
  canvas: &Canvas<'_>,
  chunk_pos: ChunkPos,
) -> (u32, u32) {
  let mut sum: u32 = 0;
  let mut count: u32 = 0;

  for (dx, dy) in HEAT_CARDINAL {
    let nx = hx as i32 + dx;
    let ny = hy as i32 + dy;

    let heat = if nx >= 0 && nx < HEAT_GRID_SIZE as i32 && ny >= 0 && ny < HEAT_GRID_SIZE as i32 {
      // Interior neighbor
      chunk.heat_cell(nx as u32, ny as u32)
    } else {
      // Cross-chunk neighbor
      let neighbor_chunk_pos = ChunkPos::new(
        chunk_pos.x
          + if nx < 0 {
            -1
          } else if nx >= HEAT_GRID_SIZE as i32 {
            1
          } else {
            0
          },
        chunk_pos.y
          + if ny < 0 {
            -1
          } else if ny >= HEAT_GRID_SIZE as i32 {
            1
          } else {
            0
          },
      );
      let Some(n) = canvas.get(neighbor_chunk_pos) else {
        continue;
      };
      n.heat_cell(
        nx.rem_euclid(HEAT_GRID_SIZE as i32) as u32,
        ny.rem_euclid(HEAT_GRID_SIZE as i32) as u32,
      )
    };

    sum += heat as u32;
    count += 1;
  }

  (sum, count)
}

/// Propagates heat across all chunks accessible through the canvas.
///
/// For each heat cell: accumulate source heat from pixels, diffuse with
/// cardinal neighbors, apply cooling. Uses a scratch buffer per chunk to
/// avoid read-write conflicts.
///
/// Only processes active heat tiles (those marked dirty or in cooldown).
pub fn propagate_heat(
  canvas: &Canvas<'_>,
  chunk_positions: &[ChunkPos],
  materials: &Materials,
  config: &HeatConfig,
  debug_gizmos: DebugGizmos<'_>,
) {
  let grid_size = HEAT_GRID_SIZE as usize;
  let cell_count = grid_size * grid_size;
  let mut scratch = vec![0u8; cell_count];

  for &chunk_pos in chunk_positions {
    // Borrow immutably first to collect active tiles
    let Some(chunk) = canvas.get(chunk_pos) else {
      continue;
    };

    // Collect active tiles BEFORE tick (so we process tiles about to expire)
    let active_tiles: Vec<(u32, u32)> = chunk.heat_dirty.active_tiles().collect();

    if active_tiles.is_empty() {
      continue;
    }

    // Process active tiles
    for &(tx, ty) in &active_tiles {
      emit_heat_dirty_tile(debug_gizmos, chunk_pos, tx, ty);
      let hx_start = tx * HEAT_CELLS_PER_TILE;
      let hy_start = ty * HEAT_CELLS_PER_TILE;

      for hy in hy_start..hy_start + HEAT_CELLS_PER_TILE {
        for hx in hx_start..hx_start + HEAT_CELLS_PER_TILE {
          let (source, solid_count) =
            accumulate_cell_heat_sources(chunk, hx, hy, materials, config.burning_heat);

          let self_heat = chunk.heat_cell(hx, hy) as u32;
          let (neighbor_sum, neighbor_count) =
            sample_heat_neighbors(hx, hy, chunk, canvas, chunk_pos);

          let neighbor_avg = if neighbor_count > 0 {
            neighbor_sum / neighbor_count
          } else {
            0
          };

          // Heat in air (no solid pixels) dissipates 10x faster
          let effective_cooling = if solid_count == 0 {
            config.cooling_factor.powi(10)
          } else {
            config.cooling_factor
          };

          let diffused = ((self_heat + neighbor_avg) as f32 / 2.0 * effective_cooling) as u32;
          let new_temp = source.max(diffused).min(255) as u8;

          scratch[(hy * HEAT_GRID_SIZE + hx) as usize] = new_temp;
        }
      }
    }

    // Write scratch back, mark dirty tiles, and tick cooldowns
    if let Some(chunk) = canvas.get_mut(chunk_pos) {
      // Write scratch values for tiles we processed
      for &(tx, ty) in &active_tiles {
        let hx_start = tx * HEAT_CELLS_PER_TILE;
        let hy_start = ty * HEAT_CELLS_PER_TILE;

        for hy in hy_start..hy_start + HEAT_CELLS_PER_TILE {
          for hx in hx_start..hx_start + HEAT_CELLS_PER_TILE {
            let idx = (hy * HEAT_GRID_SIZE + hx) as usize;
            let new_temp = scratch[idx];
            chunk.heat[idx] = new_temp;

            // Keep tile active if heat remains, also wake neighbors for diffusion
            if new_temp > 0 {
              chunk.heat_dirty.mark_dirty(hx, hy);
            }
          }
        }
      }

      // Tick cooldowns AFTER processing
      chunk.heat_dirty.tick();
    }

    // Reset scratch for next chunk
    scratch.fill(0);
  }
}

/// Ignites flammable pixels within a single heat cell that exceed their
/// threshold. Returns true if any pixel was ignited.
fn ignite_cell_pixels(
  chunk: &mut Chunk,
  hx: u32,
  hy: u32,
  heat: u8,
  materials: &Materials,
) -> bool {
  let px_base_x = hx * HEAT_CELL_SIZE;
  let px_base_y = hy * HEAT_CELL_SIZE;
  let mut ignited = false;

  for dy in 0..HEAT_CELL_SIZE {
    for dx in 0..HEAT_CELL_SIZE {
      let px = px_base_x + dx;
      let py = px_base_y + dy;
      let pixel = chunk.pixels[(px, py)];

      if pixel.is_void() {
        continue;
      }

      let mat = materials.get(pixel.material);
      let should_ignite = mat.ignition_threshold > 0
        && heat >= mat.ignition_threshold
        && !pixel.flags.contains(PixelFlags::BURNING);

      if should_ignite {
        let p = &mut chunk.pixels[(px, py)];
        p.flags.insert(PixelFlags::BURNING | PixelFlags::DIRTY);
        chunk.mark_pixel_dirty(px, py);
        ignited = true;
      }
    }
  }

  ignited
}

/// Checks heat cells and ignites flammable pixels that exceed their threshold.
///
/// Only processes active heat tiles for efficiency.
pub fn ignite_from_heat(canvas: &Canvas<'_>, chunk_positions: &[ChunkPos], materials: &Materials) {
  for &chunk_pos in chunk_positions {
    let Some(chunk) = canvas.get(chunk_pos) else {
      continue;
    };

    // Collect active tiles
    let active_tiles: Vec<(u32, u32)> = chunk.heat_dirty.active_tiles().collect();

    let Some(chunk) = canvas.get_mut(chunk_pos) else {
      continue;
    };

    for (tx, ty) in active_tiles {
      let hx_start = tx * HEAT_CELLS_PER_TILE;
      let hy_start = ty * HEAT_CELLS_PER_TILE;

      for hy in hy_start..hy_start + HEAT_CELLS_PER_TILE {
        for hx in hx_start..hx_start + HEAT_CELLS_PER_TILE {
          let heat = chunk.heat_cell(hx, hy);
          if heat > 0 {
            let ignited = ignite_cell_pixels(chunk, hx, hy, heat, materials);
            if ignited {
              // Keep heat tile active when pixels ignite
              chunk.heat_dirty.mark_dirty(hx, hy);
            }
          }
        }
      }
    }
  }
}
