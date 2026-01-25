//! Readback system for detecting pixel destruction.
//!
//! Detects pixels destroyed by:
//! 1. External modification (brush erasure) - detected before clear/blit
//! 2. CA simulation - detected after simulation

use bevy::prelude::*;

use super::blit::compute_world_aabb;
use super::{BlittedTransform, NeedsColliderRegen, PixelBody, ShapeMaskModified};
use crate::coords::WorldPos;
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
/// flag). This must run BEFORE clear_pixel_bodies overwrites the evidence.
pub fn detect_external_erasure(
  mut commands: Commands,
  worlds: Query<&PixelWorld>,
  bodies: Query<(
    Entity,
    &PixelBody,
    &BlittedTransform,
    Option<&DestroyedPixels>,
  )>,
) {
  let Ok(world) = worlds.single() else {
    return;
  };

  for (entity, body, blitted, existing_destroyed) in bodies.iter() {
    let Some(transform) = &blitted.transform else {
      continue;
    };

    let destroyed_pixels = detect_destroyed_pixels(body, transform, world);

    if !destroyed_pixels.is_empty() {
      // Merge with any existing destroyed pixels
      let mut all_destroyed = existing_destroyed.map(|d| d.0.clone()).unwrap_or_default();
      all_destroyed.extend(destroyed_pixels);

      commands
        .entity(entity)
        .insert(DestroyedPixels(all_destroyed));
    }
  }
}

/// Detects pixels destroyed by CA simulation.
///
/// Runs after simulation to detect pixels that were destroyed during the
/// CA tick (e.g., burned, dissolved, etc.). Uses BlittedTransform to check
/// positions where pixels were written this frame.
pub fn readback_pixel_bodies(
  mut commands: Commands,
  worlds: Query<&PixelWorld>,
  bodies: Query<(
    Entity,
    &PixelBody,
    &BlittedTransform,
    Option<&DestroyedPixels>,
  )>,
) {
  let Ok(world) = worlds.single() else {
    return;
  };

  for (entity, body, blitted, existing_destroyed) in bodies.iter() {
    let Some(transform) = &blitted.transform else {
      continue;
    };

    let destroyed_pixels = detect_destroyed_pixels(body, transform, world);

    if !destroyed_pixels.is_empty() {
      // Merge with any existing destroyed pixels (from external erasure)
      let mut all_destroyed = existing_destroyed.map(|d| d.0.clone()).unwrap_or_default();

      // Deduplicate
      for pixel in destroyed_pixels {
        if !all_destroyed.contains(&pixel) {
          all_destroyed.push(pixel);
        }
      }

      commands
        .entity(entity)
        .insert(DestroyedPixels(all_destroyed));
    }
  }
}

/// Core detection logic shared by external erasure and CA readback.
fn detect_destroyed_pixels(
  body: &PixelBody,
  transform: &GlobalTransform,
  world: &PixelWorld,
) -> Vec<(u32, u32)> {
  let width = body.width() as i32;
  let height = body.height() as i32;
  let origin = body.origin;

  let aabb = compute_world_aabb(body, transform);
  let inverse = transform.affine().inverse();

  let mut destroyed = Vec::new();

  for world_y in aabb.y..(aabb.y + aabb.height as i64) {
    for world_x in aabb.x..(aabb.x + aabb.width as i64) {
      let world_point = Vec3::new(world_x as f32 + 0.5, world_y as f32 + 0.5, 0.0);
      let local_point = inverse.transform_point3(world_point);

      let local_x = (local_point.x - origin.x as f32).floor() as i32;
      let local_y = (local_point.y - origin.y as f32).floor() as i32;

      if local_x < 0 || local_x >= width || local_y < 0 || local_y >= height {
        continue;
      }

      let (lx, ly) = (local_x as u32, local_y as u32);

      if !body.is_solid(lx, ly) {
        continue;
      }

      let is_destroyed = match world.get_pixel(WorldPos::new(world_x, world_y)) {
        Some(pixel) => pixel.is_void() || !pixel.flags.contains(PixelFlags::PIXEL_BODY),
        None => true,
      };

      if is_destroyed {
        destroyed.push((lx, ly));
      }
    }
  }

  destroyed
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
