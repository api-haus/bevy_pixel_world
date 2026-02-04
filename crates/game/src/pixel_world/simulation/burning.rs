//! Burning propagation and ash transformation.
//!
//! Burning pixels spread fire to adjacent flammable pixels and
//! probabilistically transform into ash. Uses checkerboard scheduling
//! and dirty rects for efficient parallel processing.
//!
//! All probability calculations use tick-rate-independent parameters
//! (rates per second, durations) converted to per-tick probabilities.

use std::collections::HashSet;

use crate::pixel_world::coords::{ChunkPos, ColorIndex, LocalPos, TILE_SIZE, TilePos, WorldPos};
use crate::pixel_world::material::{Materials, PixelEffect};
use crate::pixel_world::pixel::{Pixel, PixelFlags};
use crate::pixel_world::primitives::HEAT_CELL_SIZE;
use crate::pixel_world::scheduling::blitter::Canvas;
use crate::pixel_world::simulation::SimContext;
use crate::pixel_world::simulation::hash::hash41uu64;

/// Cardinal neighbor offsets.
const CARDINAL: [(i64, i64); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// Applies a burn effect to a pixel.
fn apply_burn_effect(
  canvas: &Canvas<'_>,
  pos: WorldPos,
  effect: PixelEffect,
  ctx: SimContext,
  dirty_chunks: &mut HashSet<ChunkPos>,
  dirty_pixels: &mut Vec<(ChunkPos, LocalPos)>,
) {
  let (chunk_pos, local) = pos.to_chunk_and_local();
  let lx = local.x as u32;
  let ly = local.y as u32;

  let Some(chunk) = canvas.get_mut(chunk_pos) else {
    return;
  };

  match effect {
    PixelEffect::Transform(target) => {
      let color_hash = hash41uu64(ctx.seed, pos.x as u64, pos.y as u64, 0xA5A5);
      let color_idx = (color_hash % 256) as u8;
      chunk.pixels[(lx, ly)] = Pixel {
        material: target,
        color: ColorIndex(color_idx),
        damage: 0,
        flags: PixelFlags::DIRTY | PixelFlags::SOLID,
      };
    }
    PixelEffect::Destroy => {
      chunk.pixels[(lx, ly)] = Pixel::VOID;
    }
    PixelEffect::Resist => return,
  }

  chunk.mark_pixel_dirty(lx, ly);
  dirty_chunks.insert(chunk_pos);
  dirty_pixels.push((chunk_pos, local));
}

/// Attempts to spread fire from a burning pixel to its cardinal neighbors.
fn try_spread_fire(
  canvas: &Canvas<'_>,
  pos: WorldPos,
  ctx: SimContext,
  materials: &Materials,
  spread_chance: f32,
  dirty_chunks: &mut HashSet<ChunkPos>,
  dirty_pixels: &mut Vec<(ChunkPos, LocalPos)>,
) {
  const CH_SPREAD: u64 = 0xdead_beef_cafe_babe;

  for &(dx, dy) in &CARDINAL {
    let nx = pos.x + dx;
    let ny = pos.y + dy;

    // Roll against tick-rate-independent spread probability
    let spread_hash = hash41uu64(ctx.seed ^ CH_SPREAD, ctx.tick, nx as u64, ny as u64);
    let spread_roll = (spread_hash & 0xFFFF) as f32 / 65535.0;
    if spread_roll >= spread_chance {
      continue;
    }

    let target = WorldPos::new(nx, ny);
    let (target_chunk_pos, target_local) = target.to_chunk_and_local();
    let tlx = target_local.x as u32;
    let tly = target_local.y as u32;

    let Some(target_chunk) = canvas.get(target_chunk_pos) else {
      continue;
    };

    let neighbor = target_chunk.pixels[(tlx, tly)];
    if neighbor.is_void() || neighbor.flags.contains(PixelFlags::BURNING) {
      continue;
    }

    let neighbor_mat = materials.get(neighbor.material);
    if neighbor_mat.ignition_threshold == 0 {
      continue;
    }

    if let Some(tc) = canvas.get_mut(target_chunk_pos) {
      let p = &mut tc.pixels[(tlx, tly)];
      p.flags.insert(PixelFlags::BURNING | PixelFlags::DIRTY);
      tc.mark_pixel_dirty(tlx, tly);
      dirty_chunks.insert(target_chunk_pos);
      dirty_pixels.push((target_chunk_pos, target_local));

      // Mark heat tile dirty for the newly burning pixel
      let hx = tlx / HEAT_CELL_SIZE;
      let hy = tly / HEAT_CELL_SIZE;
      tc.heat_dirty.mark_dirty(hx, hy);
    }
  }
}

/// Processes a single burning pixel: spread fire and apply burn effects.
fn process_burning_pixel(
  canvas: &Canvas<'_>,
  pos: WorldPos,
  burning_ctx: &BurningContext<'_>,
  dirty_chunks: &mut HashSet<ChunkPos>,
  dirty_pixels: &mut Vec<(ChunkPos, LocalPos)>,
) {
  let (chunk_pos, local) = pos.to_chunk_and_local();
  let lx = local.x as u32;
  let ly = local.y as u32;

  let Some(chunk) = canvas.get(chunk_pos) else {
    return;
  };

  let pixel = chunk.pixels[(lx, ly)];
  if !pixel.flags.contains(PixelFlags::BURNING) {
    return;
  }

  let mat = burning_ctx.materials.get(pixel.material);

  // Check for burn effect (transform to ash, destroy, etc.)
  // Uses tick-rate-independent ash_chance derived from burn_duration_secs
  const CH_ASH: u64 = 0x1234_5678_9abc_def0;
  if let Some((effect, _material_chance)) = mat.effects.on_burn {
    let ash_hash = hash41uu64(
      burning_ctx.ctx.seed ^ CH_ASH,
      burning_ctx.ctx.tick,
      pos.x as u64,
      pos.y as u64,
    );
    let ash_roll = (ash_hash & 0xFFFF) as f32 / 65535.0;
    // Use global ash_chance for tick-rate independence
    if ash_roll < burning_ctx.ash_chance {
      apply_burn_effect(
        canvas,
        pos,
        effect,
        burning_ctx.ctx,
        dirty_chunks,
        dirty_pixels,
      );
      return;
    }
  }

  // Try to spread fire to neighbors
  try_spread_fire(
    canvas,
    pos,
    burning_ctx.ctx,
    burning_ctx.materials,
    burning_ctx.spread_chance,
    dirty_chunks,
    dirty_pixels,
  );
}

/// Context for burning simulation within a tile.
///
/// Contains pre-computed per-tick probabilities derived from
/// tick-rate-independent configuration values.
pub struct BurningContext<'a> {
  pub materials: &'a Materials,
  pub ctx: SimContext,
  /// Per-tick probability of spreading fire to a single neighbor.
  /// Derived from spread_rate / (num_neighbors * burning_tps).
  pub spread_chance: f32,
  /// Per-tick probability of ash transformation.
  /// Derived from 1 / (burn_duration_secs * burning_tps).
  pub ash_chance: f32,
}

/// Processes burning propagation for a single tile using dirty bounds.
///
/// Only processes pixels within the tile's dirty rect, respecting
/// checkerboard scheduling for thread safety.
pub fn process_tile_burning(
  canvas: &Canvas<'_>,
  tile: TilePos,
  bounds: (u8, u8, u8, u8),
  jitter: (i64, i64),
  burning_ctx: &BurningContext<'_>,
  dirty_chunks: &mut HashSet<ChunkPos>,
  dirty_pixels: &mut Vec<(ChunkPos, LocalPos)>,
) {
  let tile_size = TILE_SIZE as i64;
  let base_x = tile.x * tile_size + jitter.0;
  let base_y = tile.y * tile_size + jitter.1;

  let (min_x, min_y, max_x, max_y) = bounds;

  for local_y in (min_y as i64)..=(max_y as i64) {
    for local_x in (min_x as i64)..=(max_x as i64) {
      let pos = WorldPos::new(base_x + local_x, base_y + local_y);
      process_burning_pixel(canvas, pos, burning_ctx, dirty_chunks, dirty_pixels);
    }
  }
}
