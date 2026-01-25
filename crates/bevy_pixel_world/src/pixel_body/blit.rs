//! Blit and clear systems for pixel bodies.
//!
//! These systems write pixel body content to the Canvas before CA simulation
//! and clear it afterward. Displacement is integrated: clear collects void
//! positions, blit swaps displaced pixels into those voids.

use bevy::prelude::*;

use super::PixelBody;
use crate::coords::{WorldPos, WorldRect};

/// Maps a solid body pixel to its world position and local coordinates.
pub(super) struct BodyPixelMapping {
  pub world_pos: WorldPos,
  pub local_x: u32,
  pub local_y: u32,
}

/// Iterates over all solid pixels in a body, calling `f` for each with its
/// world and local coords.
///
/// This encapsulates the AABB iteration and inverse transform calculation
/// that's common to blit, clear, and readback operations.
#[inline]
pub(super) fn for_each_body_pixel<F>(body: &PixelBody, transform: &GlobalTransform, mut f: F)
where
  F: FnMut(BodyPixelMapping),
{
  let width = body.width() as i32;
  let height = body.height() as i32;
  let origin = body.origin;

  let aabb = compute_world_aabb(body, transform);
  let inverse = transform.affine().inverse();

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

      f(BodyPixelMapping {
        world_pos: WorldPos::new(world_x, world_y),
        local_x: lx,
        local_y: ly,
      });
    }
  }
}
use crate::debug_shim::GizmosParam;
use crate::material::{Materials, PhysicsState, ids as material_ids};
use crate::pixel::{Pixel, PixelFlags};
use crate::world::PixelWorld;

/// Stores the transform used during the last blit operation.
///
/// This allows the clear system to remove pixels from the correct positions
/// even after physics has moved the body.
#[derive(Component, Default)]
pub struct LastBlitTransform {
  /// The affine transform used during the last blit.
  pub transform: Option<GlobalTransform>,
}

/// Clears and blits all pixel bodies with proper per-body displacement.
///
/// For each body:
/// 1. Clear at old position (if we have a previous transform), collecting voids
/// 2. Blit at new position, using only THIS body's voids for displacement
///
/// This combined approach ensures each body only uses its own voids for
/// displacement, preventing cross-body contamination that caused water trails.
pub fn update_pixel_bodies(
  mut commands: Commands,
  mut worlds: Query<&mut PixelWorld>,
  mut bodies: Query<(
    Entity,
    &PixelBody,
    &GlobalTransform,
    Option<&mut LastBlitTransform>,
  )>,
  materials: Res<Materials>,
  gizmos: GizmosParam,
) {
  let Ok(mut world) = worlds.single_mut() else {
    return;
  };

  for (entity, body, transform, blitted) in bodies.iter_mut() {
    // Per-body displacement tracking: cleared positions become displacement targets
    let mut displacement_targets = Vec::new();

    // Clear at old position (if we have a previous transform)
    if let Some(ref bt) = blitted {
      if let Some(old_transform) = &bt.transform {
        clear_single_body(
          &mut world,
          body,
          old_transform,
          &mut displacement_targets,
          gizmos.get(),
        );
      }
    }

    // Blit at new position, using cleared positions as displacement targets
    blit_single_body(
      &mut world,
      body,
      transform,
      &mut displacement_targets,
      &materials,
      gizmos.get(),
    );

    // Update LastBlitTransform
    match blitted {
      Some(mut bt) => {
        bt.transform = Some(*transform);
      }
      None => {
        commands.entity(entity).insert(LastBlitTransform {
          transform: Some(*transform),
        });
      }
    }
  }
}

/// Computes the axis-aligned bounding box of a rotated pixel body in world
/// space.
pub(crate) fn compute_world_aabb(body: &PixelBody, transform: &GlobalTransform) -> WorldRect {
  let width = body.width() as f32;
  let height = body.height() as f32;
  let ox = body.origin.x as f32;
  let oy = body.origin.y as f32;

  let corners = [
    Vec3::new(ox, oy, 0.0),
    Vec3::new(ox + width, oy, 0.0),
    Vec3::new(ox, oy + height, 0.0),
    Vec3::new(ox + width, oy + height, 0.0),
  ];

  let (mut min_x, mut max_x) = (f32::INFINITY, f32::NEG_INFINITY);
  let (mut min_y, mut max_y) = (f32::INFINITY, f32::NEG_INFINITY);
  for c in corners {
    let w = transform.transform_point(c);
    min_x = min_x.min(w.x);
    max_x = max_x.max(w.x);
    min_y = min_y.min(w.y);
    max_y = max_y.max(w.y);
  }

  WorldRect::new(
    min_x.floor() as i64,
    min_y.floor() as i64,
    (max_x.ceil() - min_x.floor()) as u32 + 1,
    (max_y.ceil() - min_y.floor()) as u32 + 1,
  )
}

/// Attempts to displace a fluid pixel at `pos` into one of the
/// `displacement_targets`.
///
/// Returns true if displacement occurred, false if the pixel wasn't a fluid or
/// no valid target was available.
fn try_displace_fluid(
  world: &mut PixelWorld,
  pos: WorldPos,
  displacement_targets: &mut Vec<WorldPos>,
  materials: &Materials,
  debug_gizmos: crate::debug_shim::DebugGizmos<'_>,
) -> bool {
  let Some(existing) = world.get_pixel(pos) else {
    return false;
  };

  if existing.is_void() || existing.flags.contains(PixelFlags::PIXEL_BODY) {
    return false;
  }

  // Only displace fluids (liquid/gas), not solids or powders
  let mat = materials.get(existing.material);
  if !matches!(mat.state, PhysicsState::Liquid | PhysicsState::Gas) {
    return false;
  }

  // Find a void that isn't already occupied by a body pixel
  while let Some(void_pos) = displacement_targets.pop() {
    if let Some(void_pixel) = world.get_pixel(void_pos) {
      if void_pixel.flags.contains(PixelFlags::PIXEL_BODY) {
        continue; // Skip - already has a body pixel
      }
    }
    world.set_pixel(void_pos, *existing, debug_gizmos);
    // Mark displaced pixel as simulation-dirty so it participates in CA
    world.mark_pixel_sim_dirty(void_pos);
    return true;
  }

  false
}

/// Writes a single pixel body to the world canvas using inverse transform.
///
/// Iterates world pixels in the body's AABB and maps each back to local space.
/// This guarantees every world pixel in the body's footprint is filled.
///
/// When writing over non-void, non-body material, swaps that material into
/// a void position from `displacement_targets` (displacement).
pub(super) fn blit_single_body(
  world: &mut PixelWorld,
  body: &PixelBody,
  transform: &GlobalTransform,
  displacement_targets: &mut Vec<WorldPos>,
  materials: &Materials,
  debug_gizmos: crate::debug_shim::DebugGizmos<'_>,
) {
  // Collect pixels to blit (can't mutate world while iterating)
  let mut pixels_to_blit = Vec::new();

  for_each_body_pixel(body, transform, |mapping| {
    if let Some(pixel) = body.get_pixel(mapping.local_x, mapping.local_y) {
      pixels_to_blit.push((mapping.world_pos, *pixel));
    }
  });

  // Apply displacement and blit
  for (pos, pixel) in pixels_to_blit {
    try_displace_fluid(world, pos, displacement_targets, materials, debug_gizmos);

    let mut pixel_with_flag = pixel;
    pixel_with_flag.flags.insert(PixelFlags::PIXEL_BODY);
    world.set_pixel(pos, pixel_with_flag, debug_gizmos);
  }
}

/// Writes a single pixel body without displacement.
///
/// Use this variant when displacement isn't needed (e.g., fragment blitting).
pub(super) fn blit_single_body_no_displacement(
  world: &mut PixelWorld,
  body: &PixelBody,
  transform: &GlobalTransform,
  debug_gizmos: crate::debug_shim::DebugGizmos<'_>,
) {
  for_each_body_pixel(body, transform, |mapping| {
    let Some(pixel) = body.get_pixel(mapping.local_x, mapping.local_y) else {
      return;
    };

    let mut pixel_with_flag = *pixel;
    pixel_with_flag.flags.insert(PixelFlags::PIXEL_BODY);
    world.set_pixel(mapping.world_pos, pixel_with_flag, debug_gizmos);
  });
}

/// Clears a single pixel body from the world canvas using inverse transform.
///
/// Uses the same AABB iteration as blit to ensure all written pixels are
/// cleared. Only clears positions that have PIXEL_BODY flag (actual body
/// pixels). Pushes cleared positions to `cleared_positions` for displacement
/// swaps.
pub(super) fn clear_single_body(
  world: &mut PixelWorld,
  body: &PixelBody,
  transform: &GlobalTransform,
  cleared_positions: &mut Vec<WorldPos>,
  debug_gizmos: crate::debug_shim::DebugGizmos<'_>,
) {
  let void = Pixel::new(material_ids::VOID, crate::coords::ColorIndex(0));

  for_each_body_pixel(body, transform, |mapping| {
    // Only clear if this position actually has a body pixel (PIXEL_BODY flag).
    // This ensures we only create voids where body pixels were, preserving
    // any material that might have appeared there (defensive check).
    let Some(existing) = world.get_pixel(mapping.world_pos) else {
      return;
    };
    if !existing.flags.contains(PixelFlags::PIXEL_BODY) {
      return;
    }

    cleared_positions.push(mapping.world_pos);
    world.set_pixel(mapping.world_pos, void, debug_gizmos);
  });
}

/// Clears a single pixel body without tracking cleared positions.
///
/// Use this variant when displacement isn't needed (e.g., split cleanup).
pub(super) fn clear_single_body_no_tracking(
  world: &mut PixelWorld,
  body: &PixelBody,
  transform: &GlobalTransform,
  debug_gizmos: crate::debug_shim::DebugGizmos<'_>,
) {
  let void = Pixel::new(material_ids::VOID, crate::coords::ColorIndex(0));

  for_each_body_pixel(body, transform, |mapping| {
    let Some(existing) = world.get_pixel(mapping.world_pos) else {
      return;
    };
    if !existing.flags.contains(PixelFlags::PIXEL_BODY) {
      return;
    }

    world.set_pixel(mapping.world_pos, void, debug_gizmos);
  });
}
