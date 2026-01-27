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

  // Collect body data for parallel processing
  let body_data: Vec<_> = bodies
    .iter()
    .filter(|(_, blitted, _)| !blitted.written_positions.is_empty())
    .map(|(entity, blitted, existing)| (entity, blitted, existing.map(|d| d.0.clone())))
    .collect();

  // Parallel detection phase - read-only world access
  let results: Vec<_> = body_data
    .par_iter()
    .filter_map(|(entity, blitted, existing)| {
      let destroyed_pixels = detect_destroyed_from_written(world, &blitted.written_positions);

      if destroyed_pixels.is_empty() {
        return None;
      }

      // Merge with any existing destroyed pixels (from external erasure)
      let mut seen: HashSet<(u32, u32)> = existing
        .as_deref()
        .unwrap_or_default()
        .iter()
        .copied()
        .collect();
      let mut all_destroyed: Vec<_> = seen.iter().copied().collect();

      for pixel in destroyed_pixels {
        if seen.insert(pixel) {
          all_destroyed.push(pixel);
        }
      }

      Some((*entity, all_destroyed))
    })
    .collect();

  // Sequential command application phase
  for (entity, all_destroyed) in results {
    commands
      .entity(entity)
      .insert(DestroyedPixels(all_destroyed));
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
