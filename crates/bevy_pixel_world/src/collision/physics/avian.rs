//! Avian2d physics integration for collision meshes.

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::collision::{CollisionCache, CollisionConfig, CollisionQueryPoint};
use crate::coords::{TilePos, TILE_SIZE};

use super::{PhysicsColliderRegistry, TileCollider};

/// Synchronizes physics colliders with the collision cache.
///
/// - Spawns colliders for cached meshes within proximity of query points
/// - Despawns colliders when tiles are invalidated, leave proximity, or mesh is updated
pub fn sync_physics_colliders(
    mut commands: Commands,
    mut registry: ResMut<PhysicsColliderRegistry>,
    cache: Res<CollisionCache>,
    config: Res<CollisionConfig>,
    query_points: Query<&GlobalTransform, With<CollisionQueryPoint>>,
    collider_entities: Query<(Entity, &TileCollider)>,
) {
    // Collect all tiles that should have colliders (within proximity of any query point)
    let mut desired_tiles = std::collections::HashSet::new();

    for transform in query_points.iter() {
        let pos = transform.translation();
        let center_tile = TilePos::new(
            (pos.x as i64).div_euclid(TILE_SIZE as i64),
            (pos.y as i64).div_euclid(TILE_SIZE as i64),
        );

        let radius = config.proximity_radius as i64;
        for ty in (center_tile.y - radius)..=(center_tile.y + radius) {
            for tx in (center_tile.x - radius)..=(center_tile.x + radius) {
                let tile = TilePos::new(tx, ty);
                if cache.contains(tile) {
                    desired_tiles.insert(tile);
                }
            }
        }
    }

    // Despawn colliders for tiles that are:
    // - No longer in proximity
    // - No longer in cache
    // - Have stale geometry (generation mismatch)
    let mut to_despawn = Vec::new();
    for (entity, tile_collider) in collider_entities.iter() {
        let dominated = !desired_tiles.contains(&tile_collider.tile);
        let not_cached = !cache.contains(tile_collider.tile);
        let stale = cache
            .get(tile_collider.tile)
            .map(|m| m.generation != tile_collider.generation)
            .unwrap_or(false);

        if dominated || not_cached || stale {
            to_despawn.push((entity, tile_collider.tile));
        }
    }

    for (entity, tile) in to_despawn {
        commands.entity(entity).despawn();
        registry.entities.remove(&tile);
    }

    // Spawn colliders for tiles that need them
    for tile in desired_tiles {
        if registry.entities.contains_key(&tile) {
            continue; // Already has a collider with current generation
        }

        let Some(mesh) = cache.get(tile) else {
            continue;
        };

        if mesh.triangles.is_empty() {
            continue; // No geometry to collide with
        }

        // Build compound collider from triangles
        let tile_origin = Vec2::new(
            (tile.x * TILE_SIZE as i64) as f32,
            (tile.y * TILE_SIZE as i64) as f32,
        );

        let shapes: Vec<(Vec2, f32, Collider)> = mesh
            .triangles
            .iter()
            .flat_map(|poly| {
                poly.indices.iter().filter_map(|tri| {
                    // Get vertices in local coordinates (relative to tile origin)
                    let a = poly.vertices[tri.0] - tile_origin;
                    let b = poly.vertices[tri.1] - tile_origin;
                    let c = poly.vertices[tri.2] - tile_origin;

                    // Avian2d's triangle collider
                    Some((Vec2::ZERO, 0.0, Collider::triangle(a, b, c)))
                })
            })
            .collect();

        if shapes.is_empty() {
            continue;
        }

        let collider = Collider::compound(shapes);
        let generation = mesh.generation;

        // Spawn at tile world position
        let world_pos = Vec3::new(tile_origin.x, tile_origin.y, 0.0);

        let entity = commands
            .spawn((
                RigidBody::Static,
                collider,
                Transform::from_translation(world_pos),
                TileCollider { tile, generation },
            ))
            .id();

        registry.entities.insert(tile, entity);
    }
}
