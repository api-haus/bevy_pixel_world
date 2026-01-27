//! Pixel body loading when chunks seed.
//!
//! This module queues pixel bodies from persistence when their chunk finishes
//! seeding. The actual spawning happens in
//! `body_loader::spawn_pending_pixel_bodies` after collision tiles are cached.

use bevy::prelude::*;

use super::SeededChunks;
use crate::coords::TilePos;
use crate::persistence::{PersistenceTasks, PixelBodyRecord, WorldSaveResource};
use crate::pixel_body::{PixelBodyIdGenerator, compute_transformed_aabb};

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

  let transform = GlobalTransform::from(
    Transform::from_translation(Vec3::new(record.position.x, record.position.y, 0.0))
      .with_rotation(Quat::from_rotation_z(record.rotation)),
  );

  let corners = [
    Vec3::new(-half_w, -half_h, 0.0),
    Vec3::new(half_w, -half_h, 0.0),
    Vec3::new(-half_w, half_h, 0.0),
    Vec3::new(half_w, half_h, 0.0),
  ];

  compute_transformed_aabb(corners, &transform)
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
