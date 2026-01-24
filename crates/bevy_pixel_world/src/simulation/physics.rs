//! Per-pixel physics simulation.
//!
//! Implements movement behavior for different material states (powder, liquid, gas).

use super::hash::hash41uu64;
use super::SimContext;
use crate::coords::WorldPos;
use crate::material::{Materials, PhysicsState};
use crate::scheduling::blitter::Canvas;
use crate::pixel::Pixel;

/// Returns the position to swap with, or None if pixel stays.
pub fn compute_swap(
  pos: WorldPos,
  chunks: &Canvas<'_>,
  materials: &Materials,
  ctx: SimContext,
) -> Option<WorldPos> {
  let pixel = get_pixel(chunks, pos)?;

  // Skip void
  if pixel.is_void() {
    return None;
  }

  let material = materials.get(pixel.material);

  match material.state {
    PhysicsState::Solid => None,
    PhysicsState::Powder => compute_powder_swap(pos, chunks, materials, ctx),
    PhysicsState::Liquid => compute_liquid_swap(pos, chunks, materials, ctx),
    PhysicsState::Gas => None,
  }
}

// Hash channels for independent random streams
const CH_AIR_RESISTANCE: u64 = 0x9e37_79b9_7f4a_7c15;
const CH_AIR_DRIFT: u64 = 0x3c6e_f372_fe94_f82a;

/// Computes swap target for powder (sand, soil) behavior.
fn compute_powder_swap(
  pos: WorldPos,
  chunks: &Canvas<'_>,
  materials: &Materials,
  ctx: SimContext,
) -> Option<WorldPos> {
  let src_pixel = get_pixel(chunks, pos)?;
  let src_material = materials.get(src_pixel.material);
  let src_density = src_material.density;

  // Air resistance: 1/N chance to skip this tick (particle "floats")
  if src_material.air_resistance > 0
    && hash41uu64(
      ctx.seed ^ CH_AIR_RESISTANCE,
      ctx.tick,
      pos.x as u64,
      pos.y as u64,
    ) % src_material.air_resistance as u64
      == 0
  {
    return None;
  }

  // Direction flip for diagonal movement
  let flip: i64 = if hash41uu64(ctx.seed, ctx.tick, pos.x as u64, pos.y as u64) & 1 == 0 {
    -1
  } else {
    1
  };

  // Air drift: 1/N chance to drift horizontally while falling
  let drift: i64 = if src_material.air_drift > 0
    && hash41uu64(
      ctx.seed ^ CH_AIR_DRIFT,
      ctx.tick,
      pos.x as u64,
      pos.y as u64,
    ) % src_material.air_drift as u64
      == 0
  {
    flip
  } else {
    0
  };

  try_fall_and_slide(pos, chunks, materials, src_density, drift, flip)
}

/// Computes swap target for liquid (water) behavior.
fn compute_liquid_swap(
  pos: WorldPos,
  chunks: &Canvas<'_>,
  materials: &Materials,
  ctx: SimContext,
) -> Option<WorldPos> {
  let src_pixel = get_pixel(chunks, pos)?;
  let src_material = materials.get(src_pixel.material);
  let src_density = src_material.density;

  // Air resistance: 1/N chance to skip this tick
  if src_material.air_resistance > 0
    && hash41uu64(
      ctx.seed ^ CH_AIR_RESISTANCE,
      ctx.tick,
      pos.x as u64,
      pos.y as u64,
    ) % src_material.air_resistance as u64
      == 0
  {
    return None;
  }

  // Direction flip - uniform per tick for smooth flow across tile boundaries
  let flip: i64 = if hash41uu64(ctx.seed, ctx.tick, 0, 0) & 1 == 0 {
    -1
  } else {
    1
  };

  // Air drift: 1/N chance to drift horizontally while falling
  let drift: i64 = if src_material.air_drift > 0
    && hash41uu64(
      ctx.seed ^ CH_AIR_DRIFT,
      ctx.tick,
      pos.x as u64,
      pos.y as u64,
    ) % src_material.air_drift as u64
      == 0
  {
    flip
  } else {
    0
  };

  // Try falling and diagonal sliding (shared with powder)
  if let Some(target) = try_fall_and_slide(pos, chunks, materials, src_density, drift, flip) {
    return Some(target);
  }

  // Try horizontal flow (liquid-specific)
  let dispersion = src_material.dispersion;
  if dispersion > 0 {
    let first_h = WorldPos::new(pos.x + flip, pos.y);
    let second_h = WorldPos::new(pos.x - flip, pos.y);

    if can_swap_into(chunks, materials, src_density, first_h) {
      return Some(first_h);
    }
    if can_swap_into(chunks, materials, src_density, second_h) {
      return Some(second_h);
    }
  }

  None
}

/// Attempts falling and diagonal movement for a pixel.
///
/// This encapsulates the common movement logic shared between powder and liquid.
/// The caller computes drift and flip based on material-specific behavior.
fn try_fall_and_slide(
  pos: WorldPos,
  chunks: &Canvas<'_>,
  materials: &Materials,
  src_density: u8,
  drift: i64,
  flip: i64,
) -> Option<WorldPos> {
  let down = WorldPos::new(pos.x + drift, pos.y - 1);

  // Try falling (possibly with horizontal drift)
  if can_swap_into(chunks, materials, src_density, down) {
    return Some(down);
  }

  // If drift failed, try straight down
  if drift != 0 {
    let straight_down = WorldPos::new(pos.x, pos.y - 1);
    if can_swap_into(chunks, materials, src_density, straight_down) {
      return Some(straight_down);
    }
  }

  // Try sliding diagonally
  let first = WorldPos::new(pos.x + flip, pos.y - 1);
  let second = WorldPos::new(pos.x - flip, pos.y - 1);

  if can_swap_into(chunks, materials, src_density, first) {
    return Some(first);
  }
  if can_swap_into(chunks, materials, src_density, second) {
    return Some(second);
  }

  None
}

/// Reads a pixel from chunks.
#[inline]
fn get_pixel(chunks: &Canvas<'_>, pos: WorldPos) -> Option<Pixel> {
  let (chunk_pos, local) = pos.to_chunk_and_local();
  let chunk = chunks.get(chunk_pos)?;
  Some(chunk.pixels[(local.x as u32, local.y as u32)])
}

/// Checks if a pixel with the given density can swap into the target position.
#[inline]
fn can_swap_into(
  chunks: &Canvas<'_>,
  materials: &Materials,
  src_density: u8,
  target: WorldPos,
) -> bool {
  let Some(dst_pixel) = get_pixel(chunks, target) else {
    return false; // Target chunk not loaded
  };

  if dst_pixel.is_void() {
    return true;
  }

  let dst_material = materials.get(dst_pixel.material);

  // Powders cannot be displaced - they stack on each other
  if dst_material.state == PhysicsState::Powder {
    return false;
  }

  // Can displace non-solid, non-powder if source is denser
  dst_material.state != PhysicsState::Solid && src_density > dst_material.density
}
