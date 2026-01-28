//! Heat layer propagation.
//!
//! The heat layer is a downsampled grid (1/4 resolution) per chunk. Each cell
//! accumulates heat from burning pixels and material base temperatures, then
//! diffuses to neighbors with a cooling factor.

use crate::coords::ChunkPos;
use crate::material::Materials;
use crate::pixel::PixelFlags;
use crate::primitives::{HEAT_CELL_SIZE, HEAT_GRID_SIZE};
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
        let px_base_x = hx * HEAT_CELL_SIZE;
        let px_base_y = hy * HEAT_CELL_SIZE;
        let mut source: u32 = 0;

        // Scan the 4×4 pixel region for heat sources
        for dy in 0..HEAT_CELL_SIZE {
          for dx in 0..HEAT_CELL_SIZE {
            let pixel = chunk.pixels[(px_base_x + dx, px_base_y + dy)];
            if pixel.is_void() {
              continue;
            }
            let mat = materials.get(pixel.material);
            source += mat.base_temperature as u32;
            if pixel.flags.contains(PixelFlags::BURNING) {
              source += config.burning_heat as u32;
            }
          }
        }

        // Neighbor diffusion: average of cardinal neighbors
        let self_heat = chunk.heat_cell(hx, hy) as u32;
        let mut neighbor_sum: u32 = 0;
        let mut neighbor_count: u32 = 0;

        // Interior neighbors
        if hx > 0 {
          neighbor_sum += chunk.heat_cell(hx - 1, hy) as u32;
          neighbor_count += 1;
        }
        if hx + 1 < HEAT_GRID_SIZE {
          neighbor_sum += chunk.heat_cell(hx + 1, hy) as u32;
          neighbor_count += 1;
        }
        if hy > 0 {
          neighbor_sum += chunk.heat_cell(hx, hy - 1) as u32;
          neighbor_count += 1;
        }
        if hy + 1 < HEAT_GRID_SIZE {
          neighbor_sum += chunk.heat_cell(hx, hy + 1) as u32;
          neighbor_count += 1;
        }

        // Cross-chunk neighbors at edges
        if hx == 0 {
          if let Some(n) = canvas.get(ChunkPos::new(chunk_pos.x - 1, chunk_pos.y)) {
            neighbor_sum += n.heat_cell(HEAT_GRID_SIZE - 1, hy) as u32;
            neighbor_count += 1;
          }
        }
        if hx == HEAT_GRID_SIZE - 1 {
          if let Some(n) = canvas.get(ChunkPos::new(chunk_pos.x + 1, chunk_pos.y)) {
            neighbor_sum += n.heat_cell(0, hy) as u32;
            neighbor_count += 1;
          }
        }
        if hy == 0 {
          if let Some(n) = canvas.get(ChunkPos::new(chunk_pos.x, chunk_pos.y - 1)) {
            neighbor_sum += n.heat_cell(hx, HEAT_GRID_SIZE - 1) as u32;
            neighbor_count += 1;
          }
        }
        if hy == HEAT_GRID_SIZE - 1 {
          if let Some(n) = canvas.get(ChunkPos::new(chunk_pos.x, chunk_pos.y + 1)) {
            neighbor_sum += n.heat_cell(hx, 0) as u32;
            neighbor_count += 1;
          }
        }

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

/// Checks heat cells and ignites flammable pixels that exceed their threshold.
pub fn ignite_from_heat(canvas: &Canvas<'_>, chunk_positions: &[ChunkPos], materials: &Materials) {
  for &chunk_pos in chunk_positions {
    let Some(chunk) = canvas.get_mut(chunk_pos) else {
      continue;
    };

    for hy in 0..HEAT_GRID_SIZE {
      for hx in 0..HEAT_GRID_SIZE {
        let heat = chunk.heat_cell(hx, hy);
        if heat == 0 {
          continue;
        }

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
            if mat.ignition_threshold > 0
              && heat >= mat.ignition_threshold
              && !pixel.flags.contains(PixelFlags::BURNING)
            {
              let p = &mut chunk.pixels[(px, py)];
              p.flags.insert(PixelFlags::BURNING | PixelFlags::DIRTY);
              chunk.mark_pixel_dirty(px, py);
            }
          }
        }
      }
    }
  }
}
