//! Readback system for detecting pixel destruction.
//!
//! Detects pixels destroyed by:
//! 1. External modification (brush erasure) - detected before clear/blit
//! 2. CA simulation - detected after simulation

use std::collections::HashSet;

use bevy::prelude::*;
use rayon::prelude::*;

use super::blit::detect_destroyed_from_written;
use super::{LastBlitTransform, NeedsColliderRegen, PixelBody, ShapeMaskModified};
use crate::collision::Stabilizing;
use crate::pixel::PixelFlags;
use crate::world::PixelWorld;

/// Stores pixels detected as destroyed.
///
/// Used by both `detect_external_erasure` (brush erasure) and
/// `readback_pixel_bodies` (CA destruction).
#[derive(Component, Default)]
pub struct DestroyedPixels(pub Vec<(u32, u32)>);

/// Detects pixels erased by external systems (brush, etc.) before clear/blit.
///
/// Runs at the start of the pixel body cycle, checking if any blitted pixels
/// from the previous frame have been modified (void or missing PIXEL_BODY
/// flag). This must run BEFORE update_pixel_bodies to prevent re-blitting
/// destroyed pixels.
///
/// IMPORTANT: This system immediately updates the shape_mask for destroyed
/// pixels. This prevents `update_pixel_bodies` from re-blitting pixels that
/// were just erased by the brush, which would create ghost pixels.
///
/// NOTE: Unlike readback_pixel_bodies, this system processes ALL bodies
/// including those with Stabilizing marker. External erasure (brush) should
/// work on any body regardless of its physics settling state.
///
/// Detection is parallelized across bodies since the world access is read-only.
/// Shape mask mutations are applied sequentially afterward.
pub fn detect_external_erasure(
  mut commands: Commands,
  worlds: Query<&PixelWorld>,
  mut bodies: Query<(Entity, &mut PixelBody, &LastBlitTransform)>,
) {
  let Ok(world) = worlds.single() else {
    return;
  };

  // Collect body data for parallel processing
  let body_data: Vec<_> = bodies
    .iter()
    .filter(|(_, _, blitted)| !blitted.written_positions.is_empty())
    .map(|(entity, _, blitted)| (entity, blitted))
    .collect();

  // Parallel detection phase - read-only world access
  let results: Vec<_> = body_data
    .par_iter()
    .filter_map(|&(entity, blitted)| {
      let destroyed_pixels = detect_destroyed_from_written(world, &blitted.written_positions);
      if destroyed_pixels.is_empty() {
        None
      } else {
        Some((entity, destroyed_pixels))
      }
    })
    .collect();

  // Sequential mutation phase - requires mutable PixelBody access
  for (entity, destroyed_pixels) in results {
    if let Ok((_, mut body, _)) = bodies.get_mut(entity) {
      // Immediately update shape_mask to prevent re-blitting in update_pixel_bodies
      for &(lx, ly) in &destroyed_pixels {
        body.set_solid(lx, ly, false);
      }

      // Mark for collider regen and potential splitting
      commands
        .entity(entity)
        .insert((ShapeMaskModified, NeedsColliderRegen));
    }
  }
}

/// Detects pixels destroyed by CA simulation.
///
/// Runs after simulation to detect pixels that were destroyed during the
/// CA tick (e.g., burned, dissolved, etc.). Uses LastBlitTransform to check
/// positions where pixels were written this frame.
///
/// Detection is parallelized across bodies since each body's check is
/// independent and the world access is read-only.
pub fn readback_pixel_bodies(
  mut commands: Commands,
  worlds: Query<&PixelWorld>,
  bodies: Query<(Entity, &LastBlitTransform, Option<&DestroyedPixels>), Without<Stabilizing>>,
) {
  let Ok(world) = worlds.single() else {
    return;
  };

  // Collect body data for parallel processing (no cloning - just references)
  let body_data: Vec<_> = bodies
    .iter()
    .filter(|(_, blitted, _)| !blitted.written_positions.is_empty())
    .map(|(entity, blitted, _)| (entity, blitted))
    .collect();

  // Parallel detection phase - read-only world access, no merging
  let results: Vec<_> = body_data
    .par_iter()
    .filter_map(|&(entity, blitted)| {
      let destroyed_pixels = detect_destroyed_from_written(world, &blitted.written_positions);
      (!destroyed_pixels.is_empty()).then_some((entity, destroyed_pixels))
    })
    .collect();

  // Sequential phase: merge with existing destroyed pixels (clone only when
  // needed)
  for (entity, new_destroyed) in results {
    let existing = bodies.get(entity).ok().and_then(|(_, _, e)| e);

    let all_destroyed = match existing {
      Some(e) if !e.0.is_empty() => {
        // Merge: clone existing and add new unique pixels
        let mut seen: HashSet<(u32, u32)> = e.0.iter().copied().collect();
        let mut merged = e.0.clone();
        for pixel in new_destroyed {
          if seen.insert(pixel) {
            merged.push(pixel);
          }
        }
        merged
      }
      _ => new_destroyed,
    };

    commands
      .entity(entity)
      .insert(DestroyedPixels(all_destroyed));
  }
}

/// Syncs simulation-driven changes (burning, material transformation) from
/// the world back into pixel body surfaces.
///
/// Without this, each frame the body would overwrite simulation state with
/// its stored pixels, preventing fire from persisting on bodies.
pub fn sync_simulation_to_bodies(
  worlds: Query<&PixelWorld>,
  mut bodies: Query<(&mut PixelBody, &LastBlitTransform)>,
) {
  let Ok(world) = worlds.single() else {
    return;
  };

  for (mut body, blitted) in bodies.iter_mut() {
    for wp in &blitted.written_positions {
      let Some(world_pixel) = world.get_pixel(wp.world_pos) else {
        continue;
      };

      // Skip pixels that lost their PIXEL_BODY flag (handled by destruction readback)
      if !world_pixel.flags.contains(PixelFlags::PIXEL_BODY) {
        continue;
      }

      let Some(body_pixel) = body.get_pixel(wp.local_x, wp.local_y) else {
        continue;
      };

      // Sync BURNING flag
      let body_burning = body_pixel.flags.contains(PixelFlags::BURNING);
      let world_burning = world_pixel.flags.contains(PixelFlags::BURNING);

      // Sync material changes (e.g. wood → ash from burn-to-ash)
      let material_changed = body_pixel.material != world_pixel.material;

      if body_burning != world_burning || material_changed {
        // Copy the world pixel's state back, stripping the PIXEL_BODY flag
        // (it's a canvas-only flag, not stored in body surface)
        let mut synced = *world_pixel;
        synced.flags.remove(PixelFlags::PIXEL_BODY);
        body.surface[(wp.local_x, wp.local_y)] = synced;

        // Update shape mask for this pixel (e.g. material became void-like)
        body.set_solid(wp.local_x, wp.local_y, !synced.is_void());
      }
    }
  }
}

/// Applies destroyed pixel changes to shape masks.
///
/// Consumes `DestroyedPixels` and updates the shape_mask, then inserts
/// markers for collider regeneration and potential splitting.
pub fn apply_readback_changes(
  mut commands: Commands,
  mut bodies: Query<(Entity, &mut PixelBody, &DestroyedPixels)>,
) {
  for (entity, mut body, destroyed) in bodies.iter_mut() {
    for &(lx, ly) in &destroyed.0 {
      body.set_solid(lx, ly, false);
    }

    commands
      .entity(entity)
      .remove::<DestroyedPixels>()
      .insert((ShapeMaskModified, NeedsColliderRegen));
  }
}
