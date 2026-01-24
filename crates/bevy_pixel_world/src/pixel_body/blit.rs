//! Blit and clear systems for pixel bodies.
//!
//! These systems write pixel body content to the Canvas before CA simulation
//! and clear it afterward.

use bevy::prelude::*;

use super::PixelBody;
use crate::coords::{WorldPos, WorldRect};
use crate::debug_shim::GizmosParam;
use crate::material::ids as material_ids;
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

/// Writes all pixel bodies to the world canvas.
///
/// For each pixel in the shape mask, transforms local coordinates to world
/// coordinates and writes the pixel to the PixelWorld.
///
/// Also stores the transform in `BlittedTransform` for use by clear.
///
/// This system should run before CA simulation.
pub fn blit_pixel_bodies(
  mut commands: Commands,
  mut worlds: Query<&mut PixelWorld>,
  mut bodies: Query<(
    Entity,
    &PixelBody,
    &GlobalTransform,
    Option<&mut BlittedTransform>,
  )>,
  gizmos: GizmosParam,
) {
  let Ok(mut world) = worlds.single_mut() else {
    return;
  };

  for (entity, body, transform, blitted) in bodies.iter_mut() {
    blit_single_body(&mut world, body, transform, gizmos.get());

    // Store the transform used for blitting
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
fn compute_world_aabb(body: &PixelBody, transform: &GlobalTransform) -> WorldRect {
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
fn blit_single_body(
  world: &mut PixelWorld,
  body: &PixelBody,
  transform: &GlobalTransform,
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
      let mut pixel_with_flag = *pixel;
      pixel_with_flag.flags.insert(PixelFlags::PIXEL_BODY);

      world.set_pixel(
        WorldPos::new(world_x, world_y),
        pixel_with_flag,
        debug_gizmos,
      );
    }
  }
}

/// Removes all pixel bodies from the world canvas.
///
/// Uses the stored `BlittedTransform` to clear pixels at the positions where
/// they were actually blitted, not the current physics position.
///
/// This system should run after CA simulation but before physics.
pub fn clear_pixel_bodies(
  mut worlds: Query<&mut PixelWorld>,
  bodies: Query<(&PixelBody, &BlittedTransform)>,
  gizmos: GizmosParam,
) {
  let Ok(mut world) = worlds.single_mut() else {
    return;
  };

  for (body, blitted) in bodies.iter() {
    let Some(transform) = blitted.transform.as_ref() else {
      continue;
    };
    clear_single_body(&mut world, body, transform, gizmos.get());
  }
}

/// Clears a single pixel body from the world canvas using inverse transform.
///
/// Uses the same AABB iteration as blit to ensure all written pixels are
/// cleared.
fn clear_single_body(
  world: &mut PixelWorld,
  body: &PixelBody,
  transform: &GlobalTransform,
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

      world.set_pixel(WorldPos::new(world_x, world_y), void, debug_gizmos);
    }
  }
}
