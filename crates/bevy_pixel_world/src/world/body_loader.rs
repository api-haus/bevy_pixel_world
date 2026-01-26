//! Pixel body loading and spawning systems.
//!
//! This module handles loading pixel bodies from persistence when chunks seed
//! and spawning them once their required collision tiles are cached.

use bevy::prelude::*;

use super::persistence_systems::SeededChunks;
use crate::collision::CollisionCache;
use crate::coords::{TilePos, WorldRect};
use crate::persistence::{PersistenceTasks, PixelBodyRecord, WorldSaveResource};
use crate::pixel_body::{
  DisplacementState, LastBlitTransform, Persistable, PixelBodyId, PixelBodyIdGenerator,
};

/// Entry for a body waiting to spawn.
pub(crate) struct PendingBodyEntry {
  pub record: PixelBodyRecord,
  pub required_tiles: Vec<TilePos>,
}

/// Bodies waiting for collision tiles before spawning.
#[derive(Resource, Default)]
pub struct PendingPixelBodies {
  pub(crate) entries: Vec<PendingBodyEntry>,
}

/// Computes which collision tiles a body overlaps based on its rotated AABB.
pub(crate) fn compute_required_tiles(record: &PixelBodyRecord) -> Vec<TilePos> {
  let half_w = record.width as f32 / 2.0;
  let half_h = record.height as f32 / 2.0;
  let (cos_r, sin_r) = (record.rotation.cos(), record.rotation.sin());

  let corners = [
    Vec2::new(-half_w, -half_h),
    Vec2::new(half_w, -half_h),
    Vec2::new(-half_w, half_h),
    Vec2::new(half_w, half_h),
  ];

  let (mut min_x, mut max_x) = (f32::INFINITY, f32::NEG_INFINITY);
  let (mut min_y, mut max_y) = (f32::INFINITY, f32::NEG_INFINITY);

  for c in corners {
    let rotated = Vec2::new(
      c.x * cos_r - c.y * sin_r + record.position.x,
      c.x * sin_r + c.y * cos_r + record.position.y,
    );
    min_x = min_x.min(rotated.x);
    max_x = max_x.max(rotated.x);
    min_y = min_y.min(rotated.y);
    max_y = max_y.max(rotated.y);
  }

  WorldRect::new(
    min_x.floor() as i64,
    min_y.floor() as i64,
    (max_x.ceil() - min_x.floor()) as u32 + 1,
    (max_y.ceil() - min_y.floor()) as u32 + 1,
  )
  .to_tile_range()
  .collect()
}

/// System: Queues pixel bodies when their chunk finishes seeding.
///
/// Bodies are not spawned immediately - they wait in `PendingPixelBodies` until
/// their required collision tiles are cached.
pub(crate) fn queue_pixel_bodies_on_chunk_seed(
  seeded_chunks: Res<SeededChunks>,
  save_resource: Option<Res<WorldSaveResource>>,
  mut pending: ResMut<PendingPixelBodies>,
  mut id_generator: ResMut<PixelBodyIdGenerator>,
  mut persistence_tasks: ResMut<PersistenceTasks>,
) {
  if seeded_chunks.positions.is_empty() {
    return;
  }

  let Some(save_resource) = save_resource else {
    return;
  };

  let save = match save_resource.save.read() {
    Ok(s) => s,
    Err(_) => return,
  };

  for &chunk_pos in &seeded_chunks.positions {
    let records = save.load_bodies_for_chunk(chunk_pos);

    for record in records {
      id_generator.ensure_above(record.stable_id);

      // Skip if already pending (prevents duplicate spawning)
      if pending
        .entries
        .iter()
        .any(|e| e.record.stable_id == record.stable_id)
      {
        continue;
      }

      // Check if body is empty (stale record) before queueing
      let body = record.to_pixel_body();
      if body.is_empty() {
        persistence_tasks.queue_body_remove(record.stable_id);
        continue;
      }

      // Compute which collision tiles this body needs
      let required_tiles = compute_required_tiles(&record);

      pending.entries.push(PendingBodyEntry {
        record,
        required_tiles,
      });
    }
  }
}

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
) {
  pending.entries.retain(|entry| {
    // Check if all required tiles are cached
    let ready = entry.required_tiles.iter().all(|t| cache.contains(*t));
    if !ready {
      return true; // Keep waiting
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

    #[cfg(any(feature = "avian2d", feature = "rapier2d"))]
    let Some(collider) = crate::pixel_body::generate_collider(&body) else {
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
    #[allow(unused_mut, unused_variables)]
    let mut entity_commands = commands.spawn((
      body,
      LastBlitTransform {
        transform: Some(GlobalTransform::from(transform)),
        written_positions: Vec::new(),
      },
      DisplacementState::default(),
      transform,
      PixelBodyId::new(record.stable_id),
      Persistable,
    ));

    #[cfg(feature = "avian2d")]
    entity_commands.insert((
      collider,
      avian2d::prelude::RigidBody::Dynamic,
      avian2d::prelude::LinearVelocity(record.linear_velocity),
      avian2d::prelude::AngularVelocity(record.angular_velocity),
      crate::collision::CollisionQueryPoint,
      crate::culling::StreamCulled,
    ));

    #[cfg(all(feature = "rapier2d", not(feature = "avian2d")))]
    entity_commands.insert((
      collider,
      bevy_rapier2d::prelude::RigidBody::Dynamic,
      bevy_rapier2d::prelude::Velocity {
        linvel: record.linear_velocity,
        angvel: record.angular_velocity,
      },
      crate::collision::CollisionQueryPoint,
      crate::culling::StreamCulled,
    ));

    false // Remove from pending
  });
}
