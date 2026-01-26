//! Pixel body and chunk persistence systems.
//!
//! This module handles saving and loading pixel bodies and chunks:
//! - Auto-save timer and manual save requests
//! - Saving pixel bodies when chunks unload
//! - Loading pixel bodies when chunks load
//! - Flushing pending saves to disk

use std::collections::HashSet;
use std::sync::atomic::Ordering;

use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::prelude::*;

use super::PixelWorld;
use super::control::{PersistenceComplete, PersistenceControl, RequestPersistence};
use crate::coords::{ChunkPos, WorldPos};
use crate::persistence::{
  PersistenceTasks, PixelBodyRecord, WorldSaveResource, compression::compress_lz4,
  format::StorageType,
};
use crate::pixel_body::{LastBlitTransform, Persistable, PixelBody, PixelBodyId};

/// Tracks chunks unloading this frame.
///
/// Populated by `tick_pixel_worlds` before pixel body save systems run.
/// Cleared at the start of each frame.
#[derive(Resource, Default)]
pub struct UnloadingChunks {
  /// Positions of chunks being unloaded.
  pub positions: Vec<ChunkPos>,
}

/// Tracks chunks that finished loading this frame.
///
/// Populated by `poll_seeding_tasks` when seeding completes.
/// Cleared at the start of each frame.
#[derive(Resource, Default)]
pub struct LoadedChunks {
  /// Positions of chunks that just finished loading.
  pub positions: Vec<ChunkPos>,
}

/// System: Clears chunk tracking resources at the start of each frame.
pub(crate) fn clear_chunk_tracking(
  mut unloading: ResMut<UnloadingChunks>,
  mut loaded: ResMut<LoadedChunks>,
) {
  unloading.positions.clear();
  loaded.positions.clear();
}

/// System: Ticks the auto-save timer and requests saves when the interval
/// elapses.
pub(crate) fn tick_auto_save_timer(time: Res<Time>, mut persistence: ResMut<PersistenceControl>) {
  if !persistence.auto_save.enabled {
    return;
  }

  persistence.time_since_save += time.delta();

  if persistence.time_since_save >= persistence.auto_save.interval {
    persistence.request_save();
    persistence.reset_auto_save_timer();
  }
}

/// System: Converts `RequestPersistence` messages into pending save requests.
pub(crate) fn handle_persistence_messages(
  mut messages: MessageReader<RequestPersistence>,
  mut persistence: ResMut<PersistenceControl>,
) {
  for message in messages.read() {
    if message.include_bodies {
      persistence.request_save();
    } else {
      persistence.request_chunk_save();
    }
  }
}

/// System: Processes pending save requests by queuing all modified chunks.
///
/// When a save is requested (via `PersistenceControl::request_save()` or
/// auto-save), this system queues all modified chunks to `PersistenceTasks` so
/// they get written by `flush_persistence_queue`.
pub(crate) fn process_pending_save_requests(
  persistence: Res<PersistenceControl>,
  mut persistence_tasks: ResMut<PersistenceTasks>,
  mut worlds: Query<&mut PixelWorld>,
) {
  if persistence.pending_requests.is_empty() {
    return;
  }

  let mut total_saved = 0;

  // Queue all modified chunks for saving
  for mut world in worlds.iter_mut() {
    // Collect chunks that need saving
    let to_save: Vec<_> = world
      .active_chunks()
      .filter_map(|(pos, idx)| {
        let slot = world.slot(idx);
        if slot.needs_save() {
          Some((pos, idx))
        } else {
          None
        }
      })
      .collect();

    // Queue each chunk and mark as persisted
    for (pos, idx) in to_save {
      let slot = world.slot(idx);
      let compressed = compress_lz4(slot.chunk.pixels.as_bytes());
      persistence_tasks.queue_save(pos, compressed, StorageType::Full);

      // Mark slot as persisted so we don't save again until modified
      let slot = world.slot_mut(idx);
      slot.persisted = true;
      total_saved += 1;
    }
  }

  if total_saved > 0 {
    info!("Queued {} chunks for saving", total_saved);
  }
}

/// System: Flushes pending persistence tasks to disk.
///
/// Writes queued chunk and body saves to the save file. Only runs if a
/// WorldSaveResource is present.
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
pub(crate) fn flush_persistence_queue(
  mut persistence_tasks: ResMut<PersistenceTasks>,
  save_resource: Option<ResMut<WorldSaveResource>>,
) {
  let has_chunk_saves = !persistence_tasks.save_queue.is_empty();
  let has_body_saves = !persistence_tasks.body_save_queue.is_empty();
  let has_body_removes = !persistence_tasks.body_remove_queue.is_empty();

  if !has_chunk_saves && !has_body_saves && !has_body_removes {
    return;
  }

  let Some(save_resource) = save_resource else {
    // No save file configured, discard queued operations
    persistence_tasks.save_queue.clear();
    persistence_tasks.body_save_queue.clear();
    persistence_tasks.body_remove_queue.clear();
    return;
  };

  // Process all queued saves
  let mut save = match save_resource.save.write() {
    Ok(s) => s,
    Err(e) => {
      eprintln!("Warning: failed to acquire save lock: {}", e);
      persistence_tasks.save_queue.clear();
      persistence_tasks.body_save_queue.clear();
      persistence_tasks.body_remove_queue.clear();
      return;
    }
  };

  // Save chunks
  for task in persistence_tasks.save_queue.drain(..) {
    // Create page table entry and write data
    let entry = crate::persistence::format::PageTableEntry::new(
      task.pos,
      save.data_write_pos + 4, // Skip size prefix
      task.data.len() as u32,
      task.storage_type,
    );

    // Open file and write
    if let Err(e) = write_chunk_data(&save.path, save.data_write_pos, &task.data) {
      eprintln!("Warning: failed to save chunk {:?}: {}", task.pos, e);
      continue;
    }

    // Update save state
    save.index.insert(entry);
    save.data_write_pos += 4 + task.data.len() as u64;
    save.header.chunk_count = save.index.len() as u32;
    save.dirty = true;
  }

  // Save pixel bodies
  for task in persistence_tasks.body_save_queue.drain(..) {
    if let Err(e) = save.save_body(&task.record) {
      eprintln!(
        "Warning: failed to save pixel body {}: {}",
        task.record.stable_id, e
      );
    }
  }

  // Remove pixel bodies
  for task in persistence_tasks.body_remove_queue.drain(..) {
    save.remove_body(task.stable_id);
  }

  // Flush page table periodically (every N chunks or on demand)
  if save.dirty
    && let Err(e) = save.flush()
  {
    eprintln!("Warning: failed to flush save: {}", e);
  }
}

/// Creates a PixelBodyRecord from entity components with blitted transform.
///
/// Shared helper for save systems to avoid duplicating cfg-conditional velocity
/// extraction logic.
#[allow(unused_variables)]
pub(crate) fn create_body_record_blitted(
  entity: Entity,
  body_id: &PixelBodyId,
  body: &PixelBody,
  blitted: &LastBlitTransform,
  #[cfg(feature = "avian2d")] velocities: &Query<(
    Option<&avian2d::prelude::LinearVelocity>,
    Option<&avian2d::prelude::AngularVelocity>,
  )>,
  #[cfg(all(feature = "rapier2d", not(feature = "avian2d")))] velocities: &Query<
    Option<&bevy_rapier2d::prelude::Velocity>,
  >,
) -> Option<PixelBodyRecord> {
  #[cfg(feature = "avian2d")]
  let (lin_vel, ang_vel) = velocities.get(entity).unwrap_or((None, None));

  #[cfg(all(feature = "rapier2d", not(feature = "avian2d")))]
  let velocity = velocities.get(entity).ok().flatten();

  PixelBodyRecord::from_components_blitted(
    body_id,
    body,
    blitted,
    #[cfg(feature = "avian2d")]
    lin_vel,
    #[cfg(feature = "avian2d")]
    ang_vel,
    #[cfg(all(feature = "rapier2d", not(feature = "avian2d")))]
    velocity,
    Vec::new(),
  )
}

/// System: Saves pixel bodies when their chunk unloads.
///
/// Uses the blitted transform to ensure saved position matches where pixels
/// were written.
#[allow(unused_variables)]
pub(crate) fn save_pixel_bodies_on_chunk_unload(
  mut commands: Commands,
  unloading_chunks: Res<UnloadingChunks>,
  mut persistence_tasks: ResMut<PersistenceTasks>,
  bodies: Query<(
    Entity,
    &PixelBodyId,
    &PixelBody,
    &Persistable,
    &LastBlitTransform,
  )>,
  #[cfg(feature = "avian2d")] velocities: Query<(
    Option<&avian2d::prelude::LinearVelocity>,
    Option<&avian2d::prelude::AngularVelocity>,
  )>,
  #[cfg(all(feature = "rapier2d", not(feature = "avian2d")))] velocities: Query<
    Option<&bevy_rapier2d::prelude::Velocity>,
  >,
) {
  if unloading_chunks.positions.is_empty() {
    return;
  }

  let unloading_set: HashSet<_> = unloading_chunks.positions.iter().copied().collect();

  for (entity, body_id, body, _, blitted) in bodies.iter() {
    let Some(bt) = &blitted.transform else {
      continue;
    };

    let (chunk_pos, _) =
      WorldPos::new(bt.translation().x as i64, bt.translation().y as i64).to_chunk_and_local();

    if !unloading_set.contains(&chunk_pos) {
      continue;
    }

    // If body is empty (fully erased), queue removal instead of save
    if body.is_empty() {
      persistence_tasks.queue_body_remove(body_id.value());
      commands.entity(entity).despawn();
      continue;
    }

    let Some(record) = create_body_record_blitted(
      entity,
      body_id,
      body,
      blitted,
      #[cfg(any(
        feature = "avian2d",
        all(feature = "rapier2d", not(feature = "avian2d"))
      ))]
      &velocities,
    ) else {
      continue;
    };

    persistence_tasks.queue_body_save(record);
    commands.entity(entity).despawn();
  }
}

/// System: Saves all pixel bodies when a full save is requested.
///
/// Unlike `save_pixel_bodies_on_chunk_unload`, this saves ALL bodies without
/// despawning them, used for manual saves (Ctrl+S) and auto-saves.
#[allow(unused_variables)]
pub(crate) fn save_pixel_bodies_on_request(
  persistence: Res<PersistenceControl>,
  mut persistence_tasks: ResMut<PersistenceTasks>,
  bodies: Query<(
    Entity,
    &PixelBodyId,
    &PixelBody,
    &Persistable,
    &LastBlitTransform,
  )>,
  #[cfg(feature = "avian2d")] velocities: Query<(
    Option<&avian2d::prelude::LinearVelocity>,
    Option<&avian2d::prelude::AngularVelocity>,
  )>,
  #[cfg(all(feature = "rapier2d", not(feature = "avian2d")))] velocities: Query<
    Option<&bevy_rapier2d::prelude::Velocity>,
  >,
) {
  let save_bodies = persistence
    .pending_requests
    .iter()
    .any(|req| req.include_bodies);

  if !save_bodies {
    return;
  }

  let mut count = 0;
  for (entity, body_id, body, _, blitted) in bodies.iter() {
    // If body is empty (fully erased), queue removal instead of save
    if body.is_empty() {
      persistence_tasks.queue_body_remove(body_id.value());
      continue;
    }

    let Some(record) = create_body_record_blitted(
      entity,
      body_id,
      body,
      blitted,
      #[cfg(any(
        feature = "avian2d",
        all(feature = "rapier2d", not(feature = "avian2d"))
      ))]
      &velocities,
    ) else {
      continue;
    };

    persistence_tasks.queue_body_save(record);
    count += 1;
  }

  if count > 0 {
    info!("Queued {} pixel bodies for saving", count);
  }
}

/// System: Notifies pending save requests that they have completed.
///
/// Runs after `flush_persistence_queue` to mark handles as complete and emit
/// messages.
pub(crate) fn notify_persistence_complete(
  mut persistence: ResMut<PersistenceControl>,
  mut complete_messages: MessageWriter<PersistenceComplete>,
) {
  for request in persistence.pending_requests.drain(..) {
    request.completed.store(true, Ordering::Release);
    complete_messages.write(PersistenceComplete {
      request_id: request.id,
      success: true,
      error: None,
    });
  }
}

/// Writes chunk data to the save file at the given offset.
fn write_chunk_data(path: &std::path::Path, offset: u64, data: &[u8]) -> std::io::Result<()> {
  use std::io::{Seek, SeekFrom, Write};

  let mut file = std::fs::File::options().write(true).open(path)?;
  file.seek(SeekFrom::Start(offset))?;

  // Write size prefix
  let size_bytes = (data.len() as u32).to_le_bytes();
  file.write_all(&size_bytes)?;
  file.write_all(data)?;

  Ok(())
}
