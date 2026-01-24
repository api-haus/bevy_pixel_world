//! Generic tile processor for benchmarks with runtime tile_size.
//!
//! Replicates the 2x2 checkerboard scheduling from parallel/blitter.rs
//! but accepts tile_size as a runtime parameter.

use bevy_pixel_world::primitives::Surface;
use bevy_pixel_world::Pixel;
use rayon::prelude::*;

/// Phase assignment for 2x2 checkerboard scheduling.
#[derive(Clone, Copy)]
enum Phase {
  A, // (0, 1) - top-left of 2x2
  B, // (1, 1) - top-right
  C, // (0, 0) - bottom-left
  D, // (1, 0) - bottom-right
}

impl Phase {
  fn from_tile(tx: i32, ty: i32) -> Self {
    let x_mod = tx.rem_euclid(2);
    let y_mod = ty.rem_euclid(2);
    match (x_mod, y_mod) {
      (0, 1) => Phase::A,
      (1, 1) => Phase::B,
      (0, 0) => Phase::C,
      (1, 0) => Phase::D,
      _ => unreachable!(),
    }
  }
}

/// Wrapper to share raw pointer across threads.
/// SAFETY: Checkerboard scheduling guarantees non-overlapping access.
struct SurfacePtr(*mut Surface<Pixel>);
unsafe impl Send for SurfacePtr {}
unsafe impl Sync for SurfacePtr {}

/// Blit to a surface using parallel tile processing with configurable tile
/// size.
///
/// Uses 2x2 checkerboard scheduling to avoid conflicts between adjacent tiles.
pub fn blit_with_tile_size<F>(surface: &mut Surface<Pixel>, tile_size: u32, f: F)
where
  F: Fn(u32, u32) -> Pixel + Sync,
{
  let width = surface.width();
  let height = surface.height();
  let tiles_x = (width + tile_size - 1) / tile_size;
  let tiles_y = (height + tile_size - 1) / tile_size;

  // Collect all tiles grouped by phase
  let mut phases: [Vec<(i32, i32)>; 4] = [vec![], vec![], vec![], vec![]];

  for ty in 0..tiles_y as i32 {
    for tx in 0..tiles_x as i32 {
      let phase = Phase::from_tile(tx, ty);
      let idx = match phase {
        Phase::A => 0,
        Phase::B => 1,
        Phase::C => 2,
        Phase::D => 3,
      };
      phases[idx].push((tx, ty));
    }
  }

  // Wrap raw pointer for parallel access
  // SAFETY: Checkerboard scheduling ensures non-adjacent tiles don't overlap
  let data_ptr = SurfacePtr(surface as *mut Surface<Pixel>);

  // Execute each phase sequentially, tiles within phase in parallel
  for phase_tiles in phases {
    phase_tiles.par_iter().for_each(|&(tx, ty)| {
      process_tile(&data_ptr, tx, ty, tile_size, width, height, &f);
    });
  }
}

fn process_tile<F>(
  surface_ptr: &SurfacePtr,
  tx: i32,
  ty: i32,
  tile_size: u32,
  width: u32,
  height: u32,
  f: &F,
) where
  F: Fn(u32, u32) -> Pixel + Sync,
{
  let tile_x_start = tx as u32 * tile_size;
  let tile_y_start = ty as u32 * tile_size;

  // SAFETY: Checkerboard scheduling guarantees no two parallel threads
  // access overlapping tile regions
  let surface = unsafe { &mut *surface_ptr.0 };

  for dy in 0..tile_size {
    let y = tile_y_start + dy;
    if y >= height {
      break;
    }

    for dx in 0..tile_size {
      let x = tile_x_start + dx;
      if x >= width {
        break;
      }

      surface[(x, y)] = f(x, y);
    }
  }
}
