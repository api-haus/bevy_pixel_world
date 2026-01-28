//! Bomb detonation system.
//!
//! Pixel bodies tagged with `Bomb` detonate when their outer shell catches
//! fire. Detonation destroys/transforms pixels in a blast radius, releases
//! heat, and chain-detonates nearby bombs.

use std::collections::VecDeque;

use bevy::prelude::*;

use super::PixelBody;
use crate::coords::ColorIndex;
use crate::material::Materials;
use crate::pixel::{Pixel, PixelFlags};
use crate::simulation::hash::hash41uu64;
use crate::world::{BlastHit, BlastParams, PixelWorld};

/// Marks a pixel body as a bomb that detonates when its shell catches fire.
#[derive(Component)]
pub struct Bomb {
  /// Pixels from the edge that count as "shell". Computed on `Added<Bomb>`.
  pub shell_depth: u32,
  /// Maximum blast radius in world pixels (caps ray length).
  pub blast_radius: f32,
  /// Initial explosion energy. Dissipated by material blast_resistance per
  /// pixel.
  pub blast_strength: f32,
  /// Whether this bomb has been triggered.
  pub detonated: bool,
}

/// Precomputed mask of which pixels are in the outer shell.
#[derive(Component)]
pub struct BombShellMask(pub Vec<bool>);

/// Computes the shell mask for newly added bombs via BFS from void-adjacent
/// pixels.
pub fn compute_bomb_shell(
  mut commands: Commands,
  query: Query<(Entity, &Bomb, &PixelBody), Added<Bomb>>,
) {
  for (entity, bomb, body) in &query {
    let w = body.width() as usize;
    let h = body.height() as usize;
    let len = w * h;

    // Compute shell_depth: ~10% of half-dimension, min 1
    let depth = if bomb.shell_depth == 0 {
      (w.min(h) / 20).max(1) as u32
    } else {
      bomb.shell_depth
    };

    // BFS from void-adjacent solid pixels inward
    let mut distance = vec![u32::MAX; len];
    let mut queue = VecDeque::new();

    // Seed: solid pixels adjacent to void or edge
    for y in 0..h {
      for x in 0..w {
        let idx = y * w + x;
        if !body.shape_mask[idx] {
          continue;
        }
        // Check if on edge or adjacent to void
        let on_boundary = x == 0
          || x == w - 1
          || y == 0
          || y == h - 1
          || !body.shape_mask[y * w + (x - 1)]
          || !body.shape_mask[y * w + (x + 1)]
          || !body.shape_mask[(y - 1) * w + x]
          || !body.shape_mask[(y + 1) * w + x];
        if on_boundary {
          distance[idx] = 0;
          queue.push_back((x, y));
        }
      }
    }

    // BFS
    while let Some((x, y)) = queue.pop_front() {
      let d = distance[y * w + x];
      if d >= depth {
        continue;
      }
      for (dx, dy) in [(1i32, 0), (-1, 0), (0, 1), (0, -1)] {
        let nx = x as i32 + dx;
        let ny = y as i32 + dy;
        if nx < 0 || nx >= w as i32 || ny < 0 || ny >= h as i32 {
          continue;
        }
        let (nx, ny) = (nx as usize, ny as usize);
        let nidx = ny * w + nx;
        if body.shape_mask[nidx] && distance[nidx] == u32::MAX {
          distance[nidx] = d + 1;
          queue.push_back((nx, ny));
        }
      }
    }

    // Shell = solid pixels with distance < depth
    let shell: Vec<bool> = (0..len)
      .map(|i| body.shape_mask[i] && distance[i] < depth)
      .collect();

    commands
      .entity(entity)
      .insert(BombShellMask(shell))
      .try_insert(Bomb {
        shell_depth: depth,
        ..*bomb
      });
  }
}

/// Checks if any shell pixel is burning and triggers detonation.
pub fn check_bomb_ignition(mut query: Query<(&mut Bomb, &PixelBody, &BombShellMask)>) {
  for (mut bomb, body, shell) in &mut query {
    if bomb.detonated {
      continue;
    }
    let w = body.width() as usize;
    for (i, &is_shell) in shell.0.iter().enumerate() {
      if !is_shell {
        continue;
      }
      let x = (i % w) as u32;
      let y = (i / w) as u32;
      if let Some(pixel) = body.get_pixel(x, y) {
        if pixel.flags.contains(PixelFlags::BURNING) {
          bomb.detonated = true;
          break;
        }
      }
    }
  }
}

/// Processes detonated bombs via radial ray-casting.
///
/// Delegates the ray-march to `PixelWorld::blast()`, providing a callback
/// that consumes energy by `blast_resistance` and converts pixels to
/// 90% void / 10% ash.
pub fn process_detonations(
  mut commands: Commands,
  mut bombs: Query<(Entity, &mut Bomb, &GlobalTransform)>,
  mut worlds: Query<&mut PixelWorld>,
  materials: Res<Materials>,
) {
  // Collect detonated bomb data
  let detonations: Vec<(Entity, f32, f32, Vec2)> = bombs
    .iter()
    .filter(|(_, bomb, _)| bomb.detonated)
    .map(|(entity, bomb, transform)| {
      (
        entity,
        bomb.blast_radius,
        bomb.blast_strength,
        transform.translation().xy(),
      )
    })
    .collect();

  if detonations.is_empty() {
    return;
  }

  let Ok(mut world) = worlds.single_mut() else {
    return;
  };

  for &(_entity, radius, strength, center) in &detonations {
    let params = BlastParams {
      center,
      strength,
      max_radius: radius,
      heat_radius: radius * 4.0,
    };

    world.blast(&params, |pixel, pos| {
      let mat = materials.get(pixel.material);
      let cost = mat.effects.blast_resistance;

      // 90% void, 10% ash
      let roll = hash41uu64(0xB00B, pos.x as u64, pos.y as u64, 0xDEAD);
      let new_pixel = if roll % 10 == 0 {
        let color_idx = (roll / 10 % 256) as u8;
        Pixel {
          material: crate::material::ids::ASH,
          color: ColorIndex(color_idx),
          damage: 0,
          flags: PixelFlags::DIRTY | PixelFlags::SOLID | PixelFlags::FALLING,
        }
      } else {
        Pixel::VOID
      };

      BlastHit::Hit {
        pixel: new_pixel,
        cost,
      }
    });
  }

  // Chain-detonate nearby bombs
  let centers: Vec<(f32, Vec2)> = detonations.iter().map(|&(_, r, _, c)| (r, c)).collect();
  for (_, mut bomb, transform) in &mut bombs {
    if bomb.detonated {
      continue;
    }
    let pos = transform.translation().xy();
    for &(radius, center) in &centers {
      if center.distance(pos) <= radius {
        bomb.detonated = true;
        break;
      }
    }
  }

  // Despawn detonated entities
  for (entity, _, _, _) in &detonations {
    commands.entity(*entity).despawn();
  }
}
