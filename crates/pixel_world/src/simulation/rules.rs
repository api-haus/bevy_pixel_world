//! Per-pixel simulation rules.
//!
//! Implements movement behavior for different material states.

use crate::coords::WorldPos;
use crate::material::{Materials, PhysicsState};
use crate::parallel::blitter::ChunkAccess;
use crate::pixel::Pixel;

/// Returns the position to swap with, or None if pixel stays.
pub fn compute_swap(
  pos: WorldPos,
  chunks: &ChunkAccess<'_>,
  materials: &Materials,
) -> Option<WorldPos> {
  let pixel = get_pixel(chunks, pos)?;

  // Skip air
  if pixel.is_air() {
    return None;
  }

  let material = materials.get(pixel.material);

  match material.state {
    PhysicsState::Solid => None,
    PhysicsState::Powder => compute_powder_swap(pos, chunks, materials),
    PhysicsState::Liquid => compute_liquid_swap(pos, chunks, materials),
    PhysicsState::Gas => None,
  }
}

/// Computes swap target for powder (sand, soil) behavior.
fn compute_powder_swap(
  pos: WorldPos,
  chunks: &ChunkAccess<'_>,
  materials: &Materials,
) -> Option<WorldPos> {
  let src_pixel = get_pixel(chunks, pos)?;
  let src_density = materials.get(src_pixel.material).density;

  let down = WorldPos(pos.0, pos.1 - 1);

  // Try falling straight down
  if can_swap_into(chunks, materials, src_density, down) {
    return Some(down);
  }

  // Try sliding diagonally (randomize direction based on position for
  // determinism)
  let go_left_first = (pos.0 + pos.1) % 2 == 0;
  let down_left = WorldPos(pos.0 - 1, pos.1 - 1);
  let down_right = WorldPos(pos.0 + 1, pos.1 - 1);

  if go_left_first {
    if can_swap_into(chunks, materials, src_density, down_left) {
      return Some(down_left);
    }
    if can_swap_into(chunks, materials, src_density, down_right) {
      return Some(down_right);
    }
  } else {
    if can_swap_into(chunks, materials, src_density, down_right) {
      return Some(down_right);
    }
    if can_swap_into(chunks, materials, src_density, down_left) {
      return Some(down_left);
    }
  }

  None
}

/// Computes swap target for liquid (water) behavior.
fn compute_liquid_swap(
  pos: WorldPos,
  chunks: &ChunkAccess<'_>,
  materials: &Materials,
) -> Option<WorldPos> {
  let src_pixel = get_pixel(chunks, pos)?;
  let src_material = materials.get(src_pixel.material);
  let src_density = src_material.density;

  let down = WorldPos(pos.0, pos.1 - 1);

  // Try falling straight down
  if can_swap_into(chunks, materials, src_density, down) {
    return Some(down);
  }

  // Try sliding diagonally
  let go_left_first = (pos.0 + pos.1) % 2 == 0;
  let down_left = WorldPos(pos.0 - 1, pos.1 - 1);
  let down_right = WorldPos(pos.0 + 1, pos.1 - 1);

  if go_left_first {
    if can_swap_into(chunks, materials, src_density, down_left) {
      return Some(down_left);
    }
    if can_swap_into(chunks, materials, src_density, down_right) {
      return Some(down_right);
    }
  } else {
    if can_swap_into(chunks, materials, src_density, down_right) {
      return Some(down_right);
    }
    if can_swap_into(chunks, materials, src_density, down_left) {
      return Some(down_left);
    }
  }

  // Try horizontal flow
  let dispersion = src_material.dispersion;
  if dispersion > 0 {
    let left = WorldPos(pos.0 - 1, pos.1);
    let right = WorldPos(pos.0 + 1, pos.1);

    if go_left_first {
      if can_swap_into(chunks, materials, src_density, left) {
        return Some(left);
      }
      if can_swap_into(chunks, materials, src_density, right) {
        return Some(right);
      }
    } else {
      if can_swap_into(chunks, materials, src_density, right) {
        return Some(right);
      }
      if can_swap_into(chunks, materials, src_density, left) {
        return Some(left);
      }
    }
  }

  None
}

/// Reads a pixel from chunks.
#[inline]
fn get_pixel(chunks: &ChunkAccess<'_>, pos: WorldPos) -> Option<Pixel> {
  let (chunk_pos, local) = pos.to_chunk_and_local();
  let chunk = chunks.get(chunk_pos)?;
  Some(chunk.pixels[(local.0 as u32, local.1 as u32)])
}

/// Checks if a pixel with the given density can swap into the target position.
#[inline]
fn can_swap_into(
  chunks: &ChunkAccess<'_>,
  materials: &Materials,
  src_density: u8,
  target: WorldPos,
) -> bool {
  let Some(dst_pixel) = get_pixel(chunks, target) else {
    return false; // Target chunk not loaded
  };

  if dst_pixel.is_air() {
    return true;
  }

  let dst_material = materials.get(dst_pixel.material);
  // Can displace if source is denser and target is movable
  dst_material.state != PhysicsState::Solid && src_density > dst_material.density
}
