//! Pixel body spawning systems.
//!
//! This module handles spawning pending pixel bodies once their required
//! collision tiles are cached. For body loading/queueing, see
//! streaming/body_loading.rs.

use bevy::prelude::*;

use super::streaming::PendingPixelBodies;
use crate::pixel_world::collision::CollisionCache;
use crate::pixel_world::persistence::PersistenceTasks;
use crate::pixel_world::pixel_body::{
  DisplacementState, LastBlitTransform, Persistable, PixelBodyId,
};

/// System: Spawns pending pixel bodies when their collision tiles are ready.
///
/// Bodies wait in `PendingPixelBodies` until all required collision tiles are
/// cached, ensuring they don't fall through terrain on load.
pub(crate) fn spawn_pending_pixel_bodies(
  mut commands: Commands,
  mut pending: ResMut<PendingPixelBodies>,
  cache: Res<CollisionCache>,
  mut persistence_tasks: ResMut<PersistenceTasks>,
  existing_bodies: Query<&PixelBodyId>,
  query_points: Query<(), With<crate::pixel_world::collision::CollisionQueryPoint>>,
) {
  // Only wait for collision tiles if there are active collision query points.
  // Without physics, no tiles are generated and bodies would wait forever.
  let require_tiles = !query_points.is_empty();

  pending.entries.retain(|entry| {
    // Check if all required tiles are cached
    if require_tiles {
      let ready = entry.required_tiles.iter().all(|t| cache.contains(*t));
      if !ready {
        return true; // Keep waiting
      }
    }

    let record = &entry.record;

    // Wait for old entity with same ID to be despawned (deferred despawn)
    // This prevents duplicate bodies when a chunk unloads and reloads in
    // the same frame - the old entity's despawn is applied after this runs.
    if existing_bodies.iter().any(|id| id.0 == record.stable_id) {
      return true; // Keep waiting until old entity is gone
    }

    // All tiles ready and no duplicate - spawn the body
    let body = record.to_pixel_body();

    if body.is_empty() {
      persistence_tasks.queue_body_remove(record.stable_id);
      return false;
    }

    #[cfg(physics)]
    let Some(collider) = crate::pixel_world::pixel_body::generate_collider(&body) else {
      return false;
    };

    let transform = Transform {
      translation: record.position.extend(0.0),
      rotation: Quat::from_rotation_z(record.rotation),
      scale: Vec3::ONE,
    };

    // Spawn with Dynamic - collision is guaranteed ready
    // Initialize LastBlitTransform with actual transform so erasure detection
    // doesn't skip this body on its first frame (detect_external_erasure skips
    // bodies with None transform).
    let global_transform = GlobalTransform::from(transform);
    #[allow(unused_mut)]
    let mut entity_commands = commands.spawn((
      body,
      LastBlitTransform {
        transform: Some(global_transform),
        written_positions: Vec::new(),
      },
      DisplacementState::default(),
      transform,
      // Explicit GlobalTransform ensures correct position on first frame.
      // Without this, GlobalTransform defaults to identity until PostUpdate.
      global_transform,
      PixelBodyId::new(record.stable_id),
      Persistable,
    ));

    #[cfg(physics)]
    entity_commands.insert((
      collider,
      bevy_rapier2d::prelude::RigidBody::Dynamic,
      bevy_rapier2d::prelude::Velocity {
        linvel: record.linear_velocity,
        angvel: record.angular_velocity,
      },
      crate::pixel_world::collision::CollisionQueryPoint,
      crate::pixel_world::world::streaming::StreamCulled,
    ));

    false // Remove from pending
  });
}
