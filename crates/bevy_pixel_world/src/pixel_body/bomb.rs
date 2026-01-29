//! Bomb detonation system.
//!
//! Pixel bodies tagged with `Bomb` detonate when enough of their pixels are
//! destroyed (burned, erased, blasted, etc.). Detonation destroys/transforms
//! pixels in a blast radius, releases heat, and chain-detonates nearby bombs.

use bevy::prelude::*;

use super::PixelBody;
use crate::coords::ColorIndex;
use crate::material::Materials;
use crate::pixel::{Pixel, PixelFlags};
use crate::simulation::hash::hash41uu64;
use crate::world::{BlastHit, BlastParams, PixelWorld};

/// Marks a pixel body as a bomb that detonates when enough pixels are
/// destroyed.
#[derive(Component)]
pub struct Bomb {
  /// Fraction of pixels that must be destroyed to trigger (0.0-1.0).
  pub damage_threshold: f32,
  /// Maximum blast radius in world pixels (caps ray length).
  pub blast_radius: f32,
  /// Initial explosion energy. Dissipated by material blast_resistance per
  /// pixel.
  pub blast_strength: f32,
  /// Whether this bomb has been triggered.
  pub detonated: bool,
}

/// Tracks initial pixel count for damage-based detonation.
#[derive(Component)]
pub struct BombInitialState {
  /// Initial solid pixel count (from shape_mask at spawn).
  pub initial_pixels: u32,
}

/// Initializes bomb state by counting solid pixels on spawn.
pub fn init_bomb_state(
  mut commands: Commands,
  query: Query<(Entity, &PixelBody), (With<Bomb>, Without<BombInitialState>)>,
) {
  for (entity, body) in &query {
    let initial_pixels = body.shape_mask.iter().filter(|&&s| s).count() as u32;
    commands
      .entity(entity)
      .insert(BombInitialState { initial_pixels });
  }
}

/// Checks if enough pixels are destroyed and triggers detonation.
pub fn check_bomb_damage(mut query: Query<(&mut Bomb, &BombInitialState, &PixelBody)>) {
  for (mut bomb, initial_state, body) in &mut query {
    if bomb.detonated {
      continue;
    }

    let current_pixels = body.shape_mask.iter().filter(|&&s| s).count() as u32;
    let initial = initial_state.initial_pixels;

    if initial == 0 {
      continue;
    }

    let destroyed = initial.saturating_sub(current_pixels);
    let damage_ratio = destroyed as f32 / initial as f32;

    if damage_ratio >= bomb.damage_threshold {
      bomb.detonated = true;
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

  // Build blast params for all detonations
  let blast_params: Vec<BlastParams> = detonations
    .iter()
    .map(|&(_, radius, strength, center)| BlastParams {
      center,
      strength,
      max_radius: radius,
      heat_radius: radius * 4.0,
    })
    .collect();

  // Process all blasts in a single batched operation
  world.blast_many(&blast_params, |pixel, pos| {
    let mat = materials.get(pixel.material);
    let cost = mat.effects.blast_resistance;

    // 90% void, 10% ash
    let roll = hash41uu64(0xB00B, pos.x as u64, pos.y as u64, 0xDEAD);
    let new_pixel = if roll.is_multiple_of(10) {
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
