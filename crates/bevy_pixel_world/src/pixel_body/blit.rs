//! Blit and clear systems for pixel bodies.
//!
//! These systems write pixel body content to the Canvas before CA simulation
//! and clear it afterward. Displacement is integrated: clear collects void
//! positions, blit swaps displaced pixels into those voids.

use bevy::prelude::*;

use super::PixelBody;
use crate::coords::{WorldPos, WorldRect};
use crate::debug_shim::GizmosParam;
use crate::material::{Materials, PhysicsState, ids as material_ids};
use crate::pixel::{Pixel, PixelFlags};
use crate::world::PixelWorld;

/// Stores the transform used during the last blit operation.
///
/// This allows the clear system to remove pixels from the correct positions
/// even after physics has moved the body.
#[derive(Component, Default)]
pub struct BlittedTransform {
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
    Option<&mut BlittedTransform>,
  )>,
  materials: Res<Materials>,
  gizmos: GizmosParam,
) {
  let Ok(mut world) = worlds.single_mut() else {
    return;
  };

  for (entity, body, transform, blitted) in bodies.iter_mut() {
    // Per-body void tracking
    let mut local_voids = Vec::new();

    // Clear at old position (if we have a previous transform)
    if let Some(ref bt) = blitted {
      if let Some(old_transform) = &bt.transform {
        clear_single_body(
          &mut world,
          body,
          old_transform,
          &mut local_voids,
          gizmos.get(),
        );
      }
    }

    // Blit at new position, using only this body's voids
    blit_single_body(
      &mut world,
      body,
      transform,
      &mut local_voids,
      &materials,
      gizmos.get(),
    );

    // Update BlittedTransform
    match blitted {
      Some(mut bt) => {
        bt.transform = Some(*transform);
      }
      None => {
        commands.entity(entity).insert(BlittedTransform {
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

/// Writes a single pixel body to the world canvas using inverse transform.
///
/// Iterates world pixels in the body's AABB and maps each back to local space.
/// This guarantees every world pixel in the body's footprint is filled.
///
/// When writing over non-void, non-body material, swaps that material into
/// a void position from `void_positions` (displacement).
pub(super) fn blit_single_body(
  world: &mut PixelWorld,
  body: &PixelBody,
  transform: &GlobalTransform,
  void_positions: &mut Vec<WorldPos>,
  materials: &Materials,
  debug_gizmos: crate::debug_shim::DebugGizmos<'_>,
) {
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

      let Some(pixel) = body.get_pixel(lx, ly) else {
        continue;
      };

      let pos = WorldPos::new(world_x, world_y);

      // Check if there's existing material to displace
      if let Some(existing) = world.get_pixel(pos) {
        if !existing.is_void() && !existing.flags.contains(PixelFlags::PIXEL_BODY) {
          // Only displace fluids (liquid/gas), not solids or powders
          let mat = materials.get(existing.material);
          let is_fluid = matches!(mat.state, PhysicsState::Liquid | PhysicsState::Gas);

          if is_fluid {
            // Displace: find a void that isn't already occupied by a body pixel.
            while let Some(void_pos) = void_positions.pop() {
              if let Some(void_pixel) = world.get_pixel(void_pos) {
                if void_pixel.flags.contains(PixelFlags::PIXEL_BODY) {
                  continue; // Skip - already has a body pixel
                }
              }
              world.set_pixel(void_pos, *existing, debug_gizmos);
              // Mark displaced pixel as simulation-dirty so it participates in CA
              world.mark_pixel_sim_dirty(void_pos);
              break;
            }
          }
        }
      }

      let mut pixel_with_flag = *pixel;
      pixel_with_flag.flags.insert(PixelFlags::PIXEL_BODY);

      world.set_pixel(pos, pixel_with_flag, debug_gizmos);
    }
  }
}

/// Clears a single pixel body from the world canvas using inverse transform.
///
/// Uses the same AABB iteration as blit to ensure all written pixels are
/// cleared. Only clears positions that have PIXEL_BODY flag (actual body
/// pixels). Pushes cleared positions to `void_positions` for displacement
/// swaps.
pub(super) fn clear_single_body(
  world: &mut PixelWorld,
  body: &PixelBody,
  transform: &GlobalTransform,
  void_positions: &mut Vec<WorldPos>,
  debug_gizmos: crate::debug_shim::DebugGizmos<'_>,
) {
  let width = body.width() as i32;
  let height = body.height() as i32;
  let origin = body.origin;
  let void = Pixel::new(material_ids::VOID, crate::coords::ColorIndex(0));

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

      let pos = WorldPos::new(world_x, world_y);

      // Only clear if this position actually has a body pixel (PIXEL_BODY flag).
      // This ensures we only create voids where body pixels were, preserving
      // any material that might have appeared there (defensive check).
      let Some(existing) = world.get_pixel(pos) else {
        continue;
      };
      if !existing.flags.contains(PixelFlags::PIXEL_BODY) {
        continue;
      }

      void_positions.push(pos);
      world.set_pixel(pos, void, debug_gizmos);
    }
  }
}
