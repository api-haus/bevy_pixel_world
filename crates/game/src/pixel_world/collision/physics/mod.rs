//! Physics library integration for collision meshes.
//!
//! Provides rapier2d physics support for tile collision.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

use crate::pixel_world::collision::{CollisionCache, CollisionConfig, CollisionQueryPoint};
use crate::pixel_world::coords::{TILE_SIZE, TilePos};

/// Tracks spawned physics collider entities by tile position.
#[derive(Resource, Default)]
pub struct PhysicsColliderRegistry {
  pub entities: HashMap<TilePos, Entity>,
}

/// Marker component for tile collider entities.
#[derive(Component)]
pub struct TileCollider {
  pub tile: TilePos,
  /// Generation of the mesh when this collider was created.
  /// Used to detect when the collider needs regeneration.
  pub generation: u64,
}

/// Collects tiles within proximity of query points that have cached collision
/// meshes.
fn collect_desired_tiles(
  query_points: &Query<&GlobalTransform, With<CollisionQueryPoint>>,
  cache: &CollisionCache,
  proximity_radius: u32,
) -> HashSet<TilePos> {
  let mut desired_tiles = HashSet::new();
  let radius = proximity_radius as i64;

  for transform in query_points.iter() {
    let pos = transform.translation();
    let center_tile = TilePos::new(
      (pos.x as i64).div_euclid(TILE_SIZE as i64),
      (pos.y as i64).div_euclid(TILE_SIZE as i64),
    );

    for ty in (center_tile.y - radius)..=(center_tile.y + radius) {
      for tx in (center_tile.x - radius)..=(center_tile.x + radius) {
        let tile = TilePos::new(tx, ty);
        if cache.contains(tile) {
          desired_tiles.insert(tile);
        }
      }
    }
  }

  desired_tiles
}

/// Identifies colliders that should be despawned (out of range, not cached, or
/// stale geometry). Returns (entities to despawn, tiles that had terrain
/// changes requiring body wake).
fn find_stale_colliders(
  collider_entities: &Query<(Entity, &TileCollider)>,
  desired_tiles: &HashSet<TilePos>,
  cache: &CollisionCache,
) -> (Vec<(Entity, TilePos)>, Vec<TilePos>) {
  let mut to_despawn = Vec::new();
  let mut stale_tiles = Vec::new();

  for (entity, tile_collider) in collider_entities.iter() {
    let out_of_range = !desired_tiles.contains(&tile_collider.tile);
    let not_cached = !cache.contains(tile_collider.tile);
    let stale = cache
      .get(tile_collider.tile)
      .map(|m| m.generation != tile_collider.generation)
      .unwrap_or(false);

    if out_of_range || not_cached || stale {
      to_despawn.push((entity, tile_collider.tile));
      if stale || not_cached {
        stale_tiles.push(tile_collider.tile);
      }
    }
  }

  (to_despawn, stale_tiles)
}

/// Wakes sleeping physics bodies near tiles that had terrain changes.
fn wake_bodies_near_tiles(
  _commands: &mut Commands,
  stale_tiles: &[TilePos],
  sleeping_bodies: &mut Query<(&GlobalTransform, &mut Sleeping), With<RigidBody>>,
) {
  for (transform, mut sleeping) in sleeping_bodies.iter_mut() {
    if !sleeping.sleeping {
      continue;
    }

    let pos = transform.translation();
    let body_tile = TilePos::new(
      (pos.x as i64).div_euclid(TILE_SIZE as i64),
      (pos.y as i64).div_euclid(TILE_SIZE as i64),
    );

    let should_wake = stale_tiles.iter().any(|stale_tile| {
      (body_tile.x - stale_tile.x).abs() <= 1 && (body_tile.y - stale_tile.y).abs() <= 1
    });

    if should_wake {
      sleeping.sleeping = false;
    }
  }
}

/// Spawns physics colliders for tiles that need them.
fn spawn_tile_colliders(
  commands: &mut Commands,
  registry: &mut PhysicsColliderRegistry,
  cache: &CollisionCache,
  desired_tiles: &HashSet<TilePos>,
) {
  for &tile in desired_tiles {
    if registry.entities.contains_key(&tile) {
      continue;
    }

    let Some(mesh) = cache.get(tile) else {
      continue;
    };

    if mesh.triangles.is_empty() {
      continue;
    }

    let tile_origin = Vec2::new(
      (tile.x * TILE_SIZE as i64) as f32,
      (tile.y * TILE_SIZE as i64) as f32,
    );

    let shapes: Vec<(Vec2, f32, Collider)> = mesh
      .triangles
      .iter()
      .flat_map(|poly| {
        poly.indices.iter().filter_map(|tri| {
          let a = poly.vertices[tri.a] - tile_origin;
          let b = poly.vertices[tri.b] - tile_origin;
          let c = poly.vertices[tri.c] - tile_origin;
          // Skip degenerate triangles that crash parry2d's BVH
          let cross = (b - a).perp_dot(c - a);
          if cross.abs() > f32::EPSILON {
            Some((Vec2::ZERO, 0.0, Collider::triangle(a, b, c)))
          } else {
            None
          }
        })
      })
      .collect();

    if shapes.is_empty() {
      continue;
    }

    let collider = Collider::compound(shapes);
    let generation = mesh.generation;
    let world_pos = Vec3::new(tile_origin.x, tile_origin.y, 0.0);

    let entity = commands
      .spawn((
        RigidBody::Fixed,
        collider,
        Transform::from_translation(world_pos),
        TileCollider { tile, generation },
      ))
      .id();

    registry.entities.insert(tile, entity);
  }
}

/// Synchronizes physics colliders with the collision cache.
///
/// - Spawns colliders for cached meshes within proximity of query points
/// - Despawns colliders when tiles are invalidated, leave proximity, or mesh is
///   updated
/// - Wakes sleeping dynamic bodies near changed tiles
pub fn sync_physics_colliders(
  mut commands: Commands,
  mut registry: ResMut<PhysicsColliderRegistry>,
  cache: Res<CollisionCache>,
  config: Res<CollisionConfig>,
  query_points: Query<&GlobalTransform, With<CollisionQueryPoint>>,
  collider_entities: Query<(Entity, &TileCollider)>,
  mut sleeping_bodies: Query<(&GlobalTransform, &mut Sleeping), With<RigidBody>>,
) {
  let desired_tiles = collect_desired_tiles(&query_points, &cache, config.proximity_radius);

  let (to_despawn, stale_tiles) = find_stale_colliders(&collider_entities, &desired_tiles, &cache);

  for (entity, tile) in to_despawn {
    commands.entity(entity).despawn();
    registry.entities.remove(&tile);
  }

  if !stale_tiles.is_empty() {
    wake_bodies_near_tiles(&mut commands, &stale_tiles, &mut sleeping_bodies);
  }

  spawn_tile_colliders(&mut commands, &mut registry, &cache, &desired_tiles);
}
