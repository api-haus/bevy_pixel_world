//! Streaming window entity culling.
//!
//! Automatically disables entities marked with [`StreamCulled`] when they exit
//! the streaming window, and re-enables them when they re-enter.

use bevy::ecs::entity_disabling::Disabled;
use bevy::prelude::*;

use crate::collision::CollisionCache;
use crate::coords::{CHUNK_SIZE, TILE_SIZE, TilePos, WINDOW_HEIGHT, WINDOW_WIDTH};
use crate::world::PixelWorld;

/// Marker component for entities that should be auto-culled when outside the
/// streaming window.
///
/// Add this to physics bodies or other spatial entities that should stop
/// processing when they leave the visible area. When outside the streaming
/// window, the system adds [`Disabled`] to hide the entity from all queries
/// (including physics).
///
/// # Example
///
/// ```ignore
/// commands.spawn((
///     RigidBody::Dynamic,
///     Collider::circle(10.0),
///     Transform::from_xyz(100.0, 200.0, 0.0),
///     StreamCulled,
/// ));
/// ```
#[derive(Component, Default)]
pub struct StreamCulled;

/// Internal marker tracking that [`Disabled`] was added by the culling system.
///
/// This distinguishes system-managed disabled state from user-managed, ensuring
/// we don't accidentally re-enable entities the user intentionally disabled.
#[derive(Component)]
pub(crate) struct CulledByWindow;

/// Configuration for entity culling.
#[derive(Resource, Clone, Debug)]
pub struct CullingConfig {
  /// Whether culling is enabled. Default: true.
  pub enabled: bool,
}

impl Default for CullingConfig {
  fn default() -> Self {
    Self { enabled: true }
  }
}

impl CullingConfig {
  /// Creates a config with culling enabled.
  pub fn enabled() -> Self {
    Self { enabled: true }
  }

  /// Creates a config with culling disabled.
  pub fn disabled() -> Self {
    Self { enabled: false }
  }
}

/// Query type for entities that can be culled by the streaming window.
type CulledEntityQuery<'w, 's> = Query<
  'w,
  's,
  (Entity, &'static GlobalTransform, Has<CulledByWindow>),
  (With<StreamCulled>, Allow<Disabled>),
>;

/// System that culls entities outside the streaming window.
///
/// For each entity with [`StreamCulled`]:
/// - If outside bounds and not already culled: insert `(Disabled,
///   CulledByWindow)`
/// - If inside bounds and was culled by us: remove both
pub(crate) fn update_entity_culling(
  mut commands: Commands,
  config: Res<CullingConfig>,
  cache: Res<CollisionCache>,
  worlds: Query<&PixelWorld>,
  entities: CulledEntityQuery,
) {
  if !config.enabled {
    return;
  }

  // Get streaming window bounds from the first pixel world
  let Ok(world) = worlds.single() else {
    return;
  };

  let center = world.center();

  // Compute world-space bounds of the streaming window
  let hw = WINDOW_WIDTH as i32 / 2;
  let hh = WINDOW_HEIGHT as i32 / 2;
  let chunk_size = CHUNK_SIZE as i64;

  let min_x = (center.x - hw) as i64 * chunk_size;
  let min_y = (center.y - hh) as i64 * chunk_size;
  let max_x = min_x + (WINDOW_WIDTH as i64 * chunk_size);
  let max_y = min_y + (WINDOW_HEIGHT as i64 * chunk_size);

  for (entity, transform, is_culled) in &entities {
    let pos = transform.translation();
    let x = pos.x as i64;
    let y = pos.y as i64;

    let inside = x >= min_x && x < max_x && y >= min_y && y < max_y;

    if inside && is_culled {
      // Convert entity position to tile
      let tile = TilePos::new(
        (x as f32 / TILE_SIZE as f32).floor() as i64,
        (y as f32 / TILE_SIZE as f32).floor() as i64,
      );

      // Only re-enable if collision is ready (cached and not in-flight)
      let collision_ready = cache.contains(tile) && !cache.is_in_flight(tile);
      if collision_ready {
        commands
          .entity(entity)
          .remove::<(Disabled, CulledByWindow)>();
      }
    } else if !inside && !is_culled {
      // Cull: entity is outside and not already culled
      commands.entity(entity).insert((Disabled, CulledByWindow));
    }
  }
}
