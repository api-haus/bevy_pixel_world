//! Blit and clear systems for pixel bodies.
//!
//! These systems write pixel body content to the Canvas before CA simulation
//! and clear it afterward.

use bevy::prelude::*;

use super::PixelBody;
use crate::coords::WorldPos;
use crate::debug_shim::GizmosParam;
use crate::material::ids as material_ids;
use crate::pixel::Pixel;
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

/// Writes a single pixel body to the world canvas.
fn blit_single_body(
  world: &mut PixelWorld,
  body: &PixelBody,
  transform: &GlobalTransform,
  debug_gizmos: crate::debug_shim::DebugGizmos<'_>,
) {
  let width = body.width();
  let height = body.height();
  let origin = body.origin;

  for y in 0..height {
    for x in 0..width {
      if !body.is_solid(x, y) {
        continue;
      }

      let Some(pixel) = body.get_pixel(x, y) else {
        continue;
      };

      // Transform local coordinates to world coordinates
      let local_pos = Vec3::new(
        x as f32 + origin.x as f32 + 0.5,
        y as f32 + origin.y as f32 + 0.5,
        0.0,
      );
      let world_pos_vec = transform.transform_point(local_pos);
      let world_pos = WorldPos::new(world_pos_vec.x as i64, world_pos_vec.y as i64);

      world.set_pixel(world_pos, *pixel, debug_gizmos);
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

/// Clears a single pixel body from the world canvas.
fn clear_single_body(
  world: &mut PixelWorld,
  body: &PixelBody,
  transform: &GlobalTransform,
  debug_gizmos: crate::debug_shim::DebugGizmos<'_>,
) {
  let width = body.width();
  let height = body.height();
  let origin = body.origin;
  let void = Pixel::new(material_ids::VOID, crate::coords::ColorIndex(0));

  for y in 0..height {
    for x in 0..width {
      if !body.is_solid(x, y) {
        continue;
      }

      // Transform local coordinates to world coordinates
      let local_pos = Vec3::new(
        x as f32 + origin.x as f32 + 0.5,
        y as f32 + origin.y as f32 + 0.5,
        0.0,
      );
      let world_pos_vec = transform.transform_point(local_pos);
      let world_pos = WorldPos::new(world_pos_vec.x as i64, world_pos_vec.y as i64);

      world.set_pixel(world_pos, void, debug_gizmos);
    }
  }
}
