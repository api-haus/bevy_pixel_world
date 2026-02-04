//! Radial ray-cast blast primitive for `PixelWorld`.
//!
//! The `blast` method casts rays outward from a center point, invoking a
//! caller-supplied callback for each hit pixel. The callback controls
//! energy consumption and pixel replacement, making the ray-march
//! infrastructure reusable for different blast behaviors.
//!
//! # Parallelization Strategy
//!
//! All operations use parallel iteration via rayon:
//! - Ray marching: rays processed in parallel, each collecting mutations
//! - Heat computation: rows processed in parallel
//! - Mutation/heat application: chunks processed in parallel

use std::collections::HashMap;

use bevy::math::Vec2;
use rayon::prelude::*;

use super::PixelWorld;
use crate::pixel_world::coords::{ChunkPos, LocalPos, WorldPos};
use crate::pixel_world::pixel::Pixel;
use crate::pixel_world::scheduling::blitter::Canvas;

/// Parameters for a radial blast.
pub struct BlastParams {
  /// World-space center of the blast.
  pub center: Vec2,
  /// Initial energy per ray.
  pub strength: f32,
  /// Maximum ray length in pixels.
  pub max_radius: f32,
  /// Radius for heat injection (smooth parabolic falloff).
  pub heat_radius: f32,
}

/// What the blast callback wants to do with a hit pixel.
pub enum BlastHit {
  /// Skip this pixel, ray continues without energy cost.
  Skip,
  /// Replace pixel and consume energy. Ray stops if energy drops to zero.
  Hit { pixel: Pixel, cost: f32 },
  /// Stop the ray immediately.
  Stop,
}

/// A mutation collected during the parallel compute phase.
struct BlastMutation {
  pos: WorldPos,
  pixel: Pixel,
}

impl PixelWorld {
  /// Radial ray-cast blast from `params.center`.
  ///
  /// For each non-void pixel hit by a ray, calls `f(pixel, pos)`.
  /// The callback returns a [`BlastHit`] controlling energy consumption
  /// and pixel replacement. After all rays, awakens boundary pixels and
  /// injects heat over `heat_radius`.
  pub fn blast<F>(&mut self, params: &BlastParams, f: F)
  where
    F: Fn(&Pixel, WorldPos) -> BlastHit + Sync,
  {
    self.blast_many(std::slice::from_ref(params), f);
  }

  /// Batched radial ray-cast for multiple blasts.
  ///
  /// Processes all blasts in a single operation: one Canvas creation,
  /// all rays in parallel, single mutation apply pass. Since blasts
  /// carve material (void/ash), overlapping mutations are idempotent.
  pub fn blast_many<F>(&mut self, blasts: &[BlastParams], f: F)
  where
    F: Fn(&Pixel, WorldPos) -> BlastHit + Sync,
  {
    if blasts.is_empty() {
      return;
    }

    let dirty_chunks = {
      let chunks = self.collect_seeded_chunks();
      let canvas = Canvas::new(chunks);

      // All blasts ray-march in parallel
      let mutations: Vec<BlastMutation> = blasts
        .par_iter()
        .flat_map(|params| parallel_ray_march(params, &f, &canvas))
        .collect();

      // All blasts heat computed in parallel, merged with max
      let heat_by_chunk: HashMap<ChunkPos, Vec<(LocalPos, u8)>> = blasts
        .par_iter()
        .map(|params| compute_heat_values(params, &canvas))
        .reduce(HashMap::new, merge_heat_maps);

      // Single apply pass for all mutations
      let dirty = apply_mutations_parallel(&canvas, mutations);

      // Apply merged heat (parallel across chunks)
      apply_heat_parallel(&canvas, heat_by_chunk);

      dirty
    };

    // Mark dirty chunks (after Canvas dropped)
    for pos in dirty_chunks {
      if let Some(idx) = self.pool.index_for(pos) {
        let slot = self.pool.get_mut(idx);
        slot.dirty = true;
        slot.modified = true;
        slot.persisted = false;
      }
    }

    // Awaken boundary pixels for all blasts
    for params in blasts {
      self.awaken_boundary_pixels(params);
    }
  }

  /// Awaken boundary pixels so exposed material falls/flows.
  fn awaken_boundary_pixels(&mut self, params: &BlastParams) {
    let center = params.center;
    let radius = params.max_radius;

    for ray_idx in 0..(2.0 * std::f32::consts::PI * (radius + 2.0)).ceil() as usize {
      let angle = 2.0 * std::f32::consts::PI * ray_idx as f32
        / (2.0 * std::f32::consts::PI * (radius + 2.0)).ceil();
      for dr in [-1i32, 0, 1] {
        let step = (radius as i32 + dr).max(0);
        let wx = center.x as i64 + (angle.cos() * step as f32).round() as i64;
        let wy = center.y as i64 + (angle.sin() * step as f32).round() as i64;
        self.mark_pixel_sim_dirty(WorldPos::new(wx, wy));
      }
    }
  }
}

/// Parallel ray march phase - collects mutations without modifying world.
fn parallel_ray_march<F>(params: &BlastParams, f: &F, canvas: &Canvas<'_>) -> Vec<BlastMutation>
where
  F: Fn(&Pixel, WorldPos) -> BlastHit + Sync,
{
  let center = params.center;
  let radius = params.max_radius;
  let r = radius as i32;
  let num_rays = (2.0 * std::f32::consts::PI * radius).ceil() as usize;

  (0..num_rays)
    .into_par_iter()
    .flat_map(|ray_idx| {
      let angle = 2.0 * std::f32::consts::PI * ray_idx as f32 / num_rays as f32;
      let dir_x = angle.cos();
      let dir_y = angle.sin();
      let mut remaining = params.strength;
      let mut ray_hits = Vec::new();

      for step in 0..=r {
        let wx = center.x as i64 + (dir_x * step as f32).round() as i64;
        let wy = center.y as i64 + (dir_y * step as f32).round() as i64;
        let pos = WorldPos::new(wx, wy);

        let Some(pixel) = get_pixel_from_canvas(canvas, pos) else {
          break; // unloaded chunk
        };

        if pixel.is_void() {
          continue;
        }

        match f(&pixel, pos) {
          BlastHit::Skip => continue,
          BlastHit::Stop => break,
          BlastHit::Hit {
            pixel: new_pixel,
            cost,
          } => {
            remaining -= cost;
            ray_hits.push(BlastMutation {
              pos,
              pixel: new_pixel,
            });
            if remaining <= 0.0 {
              break;
            }
          }
        }
      }

      ray_hits
    })
    .collect()
}

/// Compute heat values in parallel and group by chunk.
fn compute_heat_values(
  params: &BlastParams,
  canvas: &Canvas<'_>,
) -> HashMap<ChunkPos, Vec<(LocalPos, u8)>> {
  let center = params.center;
  let heat_radius = params.heat_radius;
  let hr = heat_radius as i32;
  let hr_sq = heat_radius * heat_radius;
  let cx = center.x as i64;
  let cy = center.y as i64;

  // Phase 1: Parallel compute - generate all heat values
  // Note: We iterate rows in parallel, cells within row sequentially
  let heat_values: Vec<(WorldPos, u8)> = (-hr..=hr)
    .into_par_iter()
    .flat_map_iter(|dy| {
      (-hr..=hr).filter_map(move |dx| {
        let dist_sq = (dx * dx + dy * dy) as f32;
        if dist_sq > hr_sq {
          return None;
        }
        // Parabolic falloff: (1 - t²) instead of spherical (1 - sqrt(t))
        // Avoids expensive sqrt while producing visually similar smooth falloff
        let t_sq = dist_sq / hr_sq;
        let heat = ((1.0 - t_sq) * 255.0) as u8;
        if heat == 0 {
          return None;
        }
        let pos = WorldPos::new(cx + dx as i64, cy + dy as i64);
        // Only include if chunk is loaded
        if canvas.get(pos.to_chunk_and_local().0).is_some() {
          Some((pos, heat))
        } else {
          None
        }
      })
    })
    .collect();

  // Phase 2: Group by chunk
  let mut heat_by_chunk: HashMap<ChunkPos, Vec<(LocalPos, u8)>> = HashMap::new();
  for (pos, heat) in heat_values {
    let (chunk, local) = pos.to_chunk_and_local();
    heat_by_chunk.entry(chunk).or_default().push((local, heat));
  }

  heat_by_chunk
}

/// Read a pixel from the canvas (read-only access).
fn get_pixel_from_canvas(canvas: &Canvas<'_>, pos: WorldPos) -> Option<Pixel> {
  let (chunk_pos, local_pos) = pos.to_chunk_and_local();
  let chunk = canvas.get(chunk_pos)?;
  Some(chunk.pixels[(local_pos.x as u32, local_pos.y as u32)])
}

/// Merge heat maps, taking max heat for overlapping cells.
fn merge_heat_maps(
  mut a: HashMap<ChunkPos, Vec<(LocalPos, u8)>>,
  b: HashMap<ChunkPos, Vec<(LocalPos, u8)>>,
) -> HashMap<ChunkPos, Vec<(LocalPos, u8)>> {
  for (chunk, heats) in b {
    a.entry(chunk).or_default().extend(heats);
  }
  a
}

/// Apply heat values to chunks in parallel.
///
/// For overlapping heat cells, takes the maximum value.
fn apply_heat_parallel(canvas: &Canvas<'_>, heat_by_chunk: HashMap<ChunkPos, Vec<(LocalPos, u8)>>) {
  heat_by_chunk.par_iter().for_each(|(chunk_pos, heats)| {
    if let Some(chunk) = canvas.get_mut(*chunk_pos) {
      // Apply directly with max - chunk heat cells default to 0
      for &(local, heat) in heats {
        let hx = local.x as u32 / crate::pixel_world::primitives::HEAT_CELL_SIZE;
        let hy = local.y as u32 / crate::pixel_world::primitives::HEAT_CELL_SIZE;
        let cell = chunk.heat_cell_mut(hx, hy);
        *cell = (*cell).max(heat);
      }
    }
  });
}

/// Apply mutations in parallel, grouped by chunk.
///
/// Since we're only setting pixels (no cross-tile swaps), all chunks can be
/// processed in parallel without phasing.
fn apply_mutations_parallel(canvas: &Canvas<'_>, mutations: Vec<BlastMutation>) -> Vec<ChunkPos> {
  // Group mutations by chunk
  let mut by_chunk: HashMap<ChunkPos, Vec<(LocalPos, Pixel)>> = HashMap::new();
  for mutation in mutations {
    let (chunk_pos, local) = mutation.pos.to_chunk_and_local();
    by_chunk
      .entry(chunk_pos)
      .or_default()
      .push((local, mutation.pixel));
  }

  let dirty_chunks: Vec<ChunkPos> = by_chunk.keys().copied().collect();

  // Apply all chunks in parallel (no cross-chunk interaction)
  by_chunk.par_iter().for_each(|(chunk_pos, pixels)| {
    if let Some(chunk) = canvas.get_mut(*chunk_pos) {
      for &(local, pixel) in pixels {
        let lx = local.x as u32;
        let ly = local.y as u32;
        chunk.pixels[(lx, ly)] = pixel;
        chunk.mark_pixel_dirty(lx, ly);
      }
    }
  });

  dirty_chunks
}
