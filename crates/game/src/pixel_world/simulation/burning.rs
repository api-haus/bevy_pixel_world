//! Burning propagation and ash transformation.
//!
//! Burning pixels spread fire to adjacent flammable pixels and
//! probabilistically transform into ash.

use crate::pixel_world::coords::{CHUNK_SIZE, ChunkPos, ColorIndex, WorldPos};
use crate::pixel_world::material::{Materials, PixelEffect};
use crate::pixel_world::pixel::{Pixel, PixelFlags};
use crate::pixel_world::primitives::Chunk;
use crate::pixel_world::scheduling::blitter::Canvas;
use crate::pixel_world::simulation::SimContext;
use crate::pixel_world::simulation::hash::hash41uu64;

/// Cardinal neighbor offsets.
const CARDINAL: [(i64, i64); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// Applies a burn effect to a pixel, returning true if the pixel was consumed.
fn apply_burn_effect(
  chunk: &mut Chunk,
  lx: u32,
  ly: u32,
  effect: PixelEffect,
  ctx: SimContext,
  world_x: i64,
  world_y: i64,
) {
  match effect {
    PixelEffect::Transform(target) => {
      let color_hash = hash41uu64(ctx.seed, world_x as u64, world_y as u64, 0xA5A5);
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
    PixelEffect::Resist => {}
  }
  chunk.mark_pixel_dirty(lx, ly);
}

/// Attempts to spread fire from a burning pixel to its cardinal neighbors.
fn try_spread_fire(
  canvas: &Canvas<'_>,
  world_x: i64,
  world_y: i64,
  ctx: SimContext,
  materials: &Materials,
  ignite_spread_chance: f32,
) {
  const CH_SPREAD: u64 = 0xdead_beef_cafe_babe;

  for &(dx, dy) in &CARDINAL {
    let nx = world_x + dx;
    let ny = world_y + dy;

    let spread_hash = hash41uu64(ctx.seed ^ CH_SPREAD, ctx.tick, nx as u64, ny as u64);
    let spread_roll = (spread_hash & 0xFFFF) as f32 / 65535.0;
    if spread_roll >= ignite_spread_chance {
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
    }
  }
}

/// Runs burning propagation and ash transformation for a single chunk.
///
/// For each burning pixel:
/// 1. Try to spread fire to cardinal neighbors (probabilistic).
/// 2. Try to transform to ash (probabilistic).
///
/// This function should be called per-chunk. It reads neighbor chunks via
/// the Canvas for cross-boundary fire spread.
pub fn process_burning(
  canvas: &Canvas<'_>,
  chunk_pos: ChunkPos,
  materials: &Materials,
  ctx: SimContext,
  ignite_spread_chance: f32,
) {
  let chunk_size = CHUNK_SIZE as i64;
  let base_x = chunk_pos.x as i64 * chunk_size;
  let base_y = chunk_pos.y as i64 * chunk_size;

  let Some(chunk) = canvas.get(chunk_pos) else {
    return;
  };

  const CH_ASH: u64 = 0x1234_5678_9abc_def0;

  for ly in 0..CHUNK_SIZE {
    for lx in 0..CHUNK_SIZE {
      let pixel = chunk.pixels[(lx, ly)];
      if !pixel.flags.contains(PixelFlags::BURNING) {
        continue;
      }

      let world_x = base_x + lx as i64;
      let world_y = base_y + ly as i64;
      let mat = materials.get(pixel.material);

      // Check for burn effect (transform to ash, destroy, etc.)
      if let Some((effect, chance)) = mat.effects.on_burn {
        let ash_hash = hash41uu64(ctx.seed ^ CH_ASH, ctx.tick, world_x as u64, world_y as u64);
        let ash_roll = (ash_hash & 0xFFFF) as f32 / 65535.0;
        if ash_roll < chance {
          if let Some(c) = canvas.get_mut(chunk_pos) {
            apply_burn_effect(c, lx, ly, effect, ctx, world_x, world_y);
          }
          continue;
        }
      }

      try_spread_fire(
        canvas,
        world_x,
        world_y,
        ctx,
        materials,
        ignite_spread_chance,
      );
    }
  }
}
