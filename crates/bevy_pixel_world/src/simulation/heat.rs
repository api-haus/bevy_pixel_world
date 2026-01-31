//! Heat layer propagation.
//!
//! The heat layer is a downsampled grid (1/4 resolution) per chunk. Each cell
//! accumulates heat from burning pixels and material base temperatures, then
//! diffuses to neighbors with a cooling factor.

use crate::coords::ChunkPos;
use crate::material::Materials;
use crate::pixel::PixelFlags;
use crate::primitives::{Chunk, HEAT_CELL_SIZE, HEAT_GRID_SIZE};
use crate::scheduling::blitter::Canvas;

/// Configuration for heat simulation.
#[derive(bevy::prelude::Resource)]
pub struct HeatConfig {
  /// Multiplier applied during diffusion (default 0.95).
  pub cooling_factor: f32,
  /// Heat emitted per burning pixel into its heat cell (default 50).
  pub burning_heat: u8,
  /// Number of CA ticks between heat propagation steps (default 6, ~10 TPS at
  /// 60 TPS).
  pub heat_tick_interval: u32,
  /// Per-neighbor per-tick chance of fire spreading from a burning pixel
  /// (default 0.3).
  pub ignite_spread_chance: f32,
}

impl Default for HeatConfig {
  fn default() -> Self {
    Self {
      cooling_factor: 0.95,
      burning_heat: 50,
      heat_tick_interval: 6,
      ignite_spread_chance: 0.3,
    }
  }
}

/// Accumulates heat from pixel sources within a single heat cell's 4x4 region.
fn accumulate_cell_heat_sources(
  chunk: &Chunk,
  hx: u32,
  hy: u32,
  materials: &Materials,
  burning_heat: u8,
) -> u32 {
  let px_base_x = hx * HEAT_CELL_SIZE;
  let px_base_y = hy * HEAT_CELL_SIZE;
  let mut source: u32 = 0;

  for dy in 0..HEAT_CELL_SIZE {
    for dx in 0..HEAT_CELL_SIZE {
      let pixel = chunk.pixels[(px_base_x + dx, px_base_y + dy)];
      if pixel.is_void() {
        continue;
      }
      let mat = materials.get(pixel.material);
      source += mat.base_temperature as u32;
      if pixel.flags.contains(PixelFlags::BURNING) {
        source += burning_heat as u32;
      }
    }
  }

  source
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
pub fn propagate_heat(
  canvas: &Canvas<'_>,
  chunk_positions: &[ChunkPos],
  materials: &Materials,
  config: &HeatConfig,
) {
  let grid_size = HEAT_GRID_SIZE as usize;
  let cell_count = grid_size * grid_size;
  let mut scratch = vec![0u8; cell_count];

  for &chunk_pos in chunk_positions {
    let Some(chunk) = canvas.get(chunk_pos) else {
      continue;
    };

    for hy in 0..HEAT_GRID_SIZE {
      for hx in 0..HEAT_GRID_SIZE {
        let source = accumulate_cell_heat_sources(chunk, hx, hy, materials, config.burning_heat);

        let self_heat = chunk.heat_cell(hx, hy) as u32;
        let (neighbor_sum, neighbor_count) =
          sample_heat_neighbors(hx, hy, chunk, canvas, chunk_pos);

        let neighbor_avg = if neighbor_count > 0 {
          neighbor_sum / neighbor_count
        } else {
          0
        };

        let diffused = ((self_heat + neighbor_avg) as f32 / 2.0 * config.cooling_factor) as u32;
        let new_temp = source.max(diffused).min(255) as u8;

        scratch[(hy * HEAT_GRID_SIZE + hx) as usize] = new_temp;
      }
    }

    // Write scratch back via Canvas interior mutability
    if let Some(chunk) = canvas.get_mut(chunk_pos) {
      chunk.heat[..cell_count].copy_from_slice(&scratch);
    }
  }
}

/// Ignites flammable pixels within a single heat cell that exceed their
/// threshold.
fn ignite_cell_pixels(chunk: &mut Chunk, hx: u32, hy: u32, heat: u8, materials: &Materials) {
  let px_base_x = hx * HEAT_CELL_SIZE;
  let px_base_y = hy * HEAT_CELL_SIZE;

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
      }
    }
  }
}

/// Checks heat cells and ignites flammable pixels that exceed their threshold.
pub fn ignite_from_heat(canvas: &Canvas<'_>, chunk_positions: &[ChunkPos], materials: &Materials) {
  for &chunk_pos in chunk_positions {
    let Some(chunk) = canvas.get_mut(chunk_pos) else {
      continue;
    };

    for hy in 0..HEAT_GRID_SIZE {
      for hx in 0..HEAT_GRID_SIZE {
        let heat = chunk.heat_cell(hx, hy);
        if heat > 0 {
          ignite_cell_pixels(chunk, hx, hy, heat, materials);
        }
      }
    }
  }
}
