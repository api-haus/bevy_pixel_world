//! Streaming window entity culling.
//!
//! Automatically disables entities marked with [`StreamCulled`] when they exit
//! the streaming window, and re-enables them when they re-enter.

use bevy::ecs::entity_disabling::Disabled;
use bevy::prelude::*;

use crate::pixel_world::collision::CollisionCache;
use crate::pixel_world::coords::{
  CHUNK_SIZE, ChunkPos, TILE_SIZE, TilePos, WINDOW_HEIGHT, WINDOW_WIDTH,
};
use crate::pixel_world::world::PixelWorld;

/// Compute world-space bounds of the streaming window.
///
/// Returns (min_x, min_y, max_x, max_y) in world pixel coordinates.
pub fn streaming_window_bounds(center: ChunkPos) -> (i64, i64, i64, i64) {
  let hw = WINDOW_WIDTH as i32 / 2;
  let hh = WINDOW_HEIGHT as i32 / 2;
  let chunk_size = CHUNK_SIZE as i64;
  let min_x = (center.x - hw) as i64 * chunk_size;
  let min_y = (center.y - hh) as i64 * chunk_size;
  let max_x = min_x + (WINDOW_WIDTH as i64 * chunk_size);
  let max_y = min_y + (WINDOW_HEIGHT as i64 * chunk_size);
  (min_x, min_y, max_x, max_y)
}

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

/// Checks if a culled entity should be re-enabled.
///
/// An entity is re-enabled when its collision data is ready (cached and not
/// in-flight). This ensures physics interactions work correctly upon re-entry.
fn should_reenable_entity(cache: &CollisionCache, x: i64, y: i64) -> bool {
  let tile = TilePos::new(
    (x as f32 / TILE_SIZE as f32).floor() as i64,
    (y as f32 / TILE_SIZE as f32).floor() as i64,
  );
  cache.contains(tile) && !cache.is_in_flight(tile)
}

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

  let (min_x, min_y, max_x, max_y) = streaming_window_bounds(world.center());

  for (entity, transform, is_culled) in &entities {
    let pos = transform.translation();
    let x = pos.x as i64;
    let y = pos.y as i64;

    let inside = x >= min_x && x < max_x && y >= min_y && y < max_y;

    if inside && is_culled && should_reenable_entity(&cache, x, y) {
      commands
        .entity(entity)
        .remove::<(Disabled, CulledByWindow)>();
    } else if !inside && !is_culled {
      commands.entity(entity).insert((Disabled, CulledByWindow));
    }
  }
}
