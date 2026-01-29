//! Pixel body and chunk persistence systems.
//!
//! This module handles saving and loading pixel bodies and chunks:
//! - Named save files with copy-on-write semantics
//! - Saving pixel bodies when chunks unload
//! - Flushing pending saves to disk
//!
//! For chunk tracking resources (UnloadingChunks, SeededChunks) and their
//! frame reset, see the streaming module.

use std::collections::HashSet;
#[cfg(not(target_family = "wasm"))]
use std::sync::atomic::Ordering;

use bevy::ecs::entity_disabling::Disabled;
use bevy::ecs::message::MessageReader;
#[cfg(not(target_family = "wasm"))]
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;

use super::PixelWorld;
#[cfg(not(target_family = "wasm"))]
use super::control::PersistenceComplete;
use super::control::{PersistenceControl, RequestPersistence};
use super::streaming::UnloadingChunks;
use crate::coords::WorldPos;
use crate::persistence::{
  PersistenceTasks, PixelBodyRecord, compression::compress_lz4, format::StorageType,
};
use crate::pixel_body::{LastBlitTransform, Persistable, PixelBody, PixelBodyId};

/// System: Converts `RequestPersistence` messages into pending save requests.
pub(crate) fn handle_persistence_messages(
  mut messages: MessageReader<RequestPersistence>,
  persistence: Option<ResMut<PersistenceControl>>,
) {
  let Some(mut persistence) = persistence else {
    // Drain messages to avoid accumulation
    for _ in messages.read() {}
    return;
  };

  // Need an active save to process messages
  if !persistence.is_active() {
    for _ in messages.read() {}
    return;
  }

  for _message in messages.read() {
    persistence.save();
  }
}

/// System: Processes pending save requests by queuing all modified chunks.
///
/// When a save is requested (via `PersistenceControl::request_save()` or
/// auto-save), this system queues all modified chunks to `PersistenceTasks` so
/// they get written by `flush_persistence_queue`.
pub(crate) fn process_pending_save_requests(
  persistence: Option<Res<PersistenceControl>>,
  mut persistence_tasks: ResMut<PersistenceTasks>,
  mut worlds: Query<&mut PixelWorld>,
) {
  let Some(persistence) = persistence else {
    return;
  };
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
      let compressed = compress_lz4(&slot.chunk.pixels.bytes_without_body_pixels());
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

/// Returns true if there are pending persistence operations.
fn has_pending_work(tasks: &PersistenceTasks) -> bool {
  !tasks.save_queue.is_empty()
    || !tasks.body_save_queue.is_empty()
    || !tasks.body_remove_queue.is_empty()
}

/// Clears all queued operations without saving.
fn discard_queued_operations(tasks: &mut PersistenceTasks) {
  tasks.save_queue.clear();
  tasks.body_save_queue.clear();
  tasks.body_remove_queue.clear();
}

/// System: Legacy flush for persistence tasks.
///
/// With the unified async architecture, saves now go through IoDispatcher
/// via dispatch_save_task. This system is kept for backward compatibility
/// but does minimal work.
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
pub(crate) fn flush_persistence_queue(
  _persistence_tasks: ResMut<PersistenceTasks>,
  mut saving: ResMut<SavingChunks>,
  io_dispatcher: Option<Res<IoDispatcher>>,
) {
  // Both native and WASM now use IoDispatcher for saves via dispatch_save_task.
  // This system just resets the busy flag after Flush was sent.
  if !saving.busy {
    return;
  }

  // If IoDispatcher is available and ready, the Flush was already sent
  // by dispatch_save_task. Just clear the busy flag.
  if io_dispatcher.as_ref().is_some_and(|d| d.is_ready()) {
    saving.busy = false;
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

/// What to do with an entity after saving its body.
enum PostSaveAction {
  /// Despawn the entity (chunk unload).
  Despawn,
  /// Keep the entity alive (manual/auto save).
  Keep,
}

/// Iterates bodies, queuing saves or removals for each. Returns number saved.
///
/// Shared logic for both chunk-unload and request-based saves.
#[allow(unused_variables)]
fn save_matching_bodies(
  commands: &mut Commands,
  persistence_tasks: &mut PersistenceTasks,
  bodies: &Query<
    (
      Entity,
      &PixelBodyId,
      &PixelBody,
      &Persistable,
      &LastBlitTransform,
    ),
    Allow<Disabled>,
  >,
  #[cfg(feature = "avian2d")] velocities: &Query<(
    Option<&avian2d::prelude::LinearVelocity>,
    Option<&avian2d::prelude::AngularVelocity>,
  )>,
  #[cfg(all(feature = "rapier2d", not(feature = "avian2d")))] velocities: &Query<
    Option<&bevy_rapier2d::prelude::Velocity>,
  >,
  mut filter: impl FnMut(Entity, &LastBlitTransform) -> Option<PostSaveAction>,
) -> u32 {
  let mut count = 0;

  for (entity, body_id, body, _, blitted) in bodies.iter() {
    let Some(action) = filter(entity, blitted) else {
      continue;
    };

    if body.is_empty() {
      persistence_tasks.queue_body_remove(body_id.value());
      if matches!(action, PostSaveAction::Despawn) {
        commands.entity(entity).despawn();
      }
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
      velocities,
    ) else {
      continue;
    };

    persistence_tasks.queue_body_save(record);
    count += 1;

    if matches!(action, PostSaveAction::Despawn) {
      commands.entity(entity).despawn();
    }
  }

  count
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
  bodies: Query<
    (
      Entity,
      &PixelBodyId,
      &PixelBody,
      &Persistable,
      &LastBlitTransform,
    ),
    Allow<Disabled>,
  >,
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

  save_matching_bodies(
    &mut commands,
    &mut persistence_tasks,
    &bodies,
    #[cfg(any(
      feature = "avian2d",
      all(feature = "rapier2d", not(feature = "avian2d"))
    ))]
    &velocities,
    |_entity, blitted| {
      let bt = blitted.transform.as_ref()?;
      let (chunk_pos, _) =
        WorldPos::new(bt.translation().x as i64, bt.translation().y as i64).to_chunk_and_local();
      unloading_set
        .contains(&chunk_pos)
        .then_some(PostSaveAction::Despawn)
    },
  );
}

/// System: Saves all pixel bodies when a save is requested.
///
/// Unlike `save_pixel_bodies_on_chunk_unload`, this saves ALL bodies without
/// despawning them, used for manual saves (Ctrl+S) and auto-saves.
#[allow(unused_variables)]
pub(crate) fn save_pixel_bodies_on_request(
  mut commands: Commands,
  persistence: Option<Res<PersistenceControl>>,
  mut persistence_tasks: ResMut<PersistenceTasks>,
  bodies: Query<
    (
      Entity,
      &PixelBodyId,
      &PixelBody,
      &Persistable,
      &LastBlitTransform,
    ),
    Allow<Disabled>,
  >,
  #[cfg(feature = "avian2d")] velocities: Query<(
    Option<&avian2d::prelude::LinearVelocity>,
    Option<&avian2d::prelude::AngularVelocity>,
  )>,
  #[cfg(all(feature = "rapier2d", not(feature = "avian2d")))] velocities: Query<
    Option<&bevy_rapier2d::prelude::Velocity>,
  >,
) {
  let Some(persistence) = persistence else {
    return;
  };

  // Only save bodies if there are pending save requests
  if persistence.pending_requests.is_empty() {
    return;
  }

  let count = save_matching_bodies(
    &mut commands,
    &mut persistence_tasks,
    &bodies,
    #[cfg(any(
      feature = "avian2d",
      all(feature = "rapier2d", not(feature = "avian2d"))
    ))]
    &velocities,
    |_entity, _blitted| Some(PostSaveAction::Keep),
  );

  if count > 0 {
    info!("Queued {} pixel bodies for saving", count);
  }
}

/// System: Notifies pending save requests that they have completed.
///
/// Runs after save systems complete. Only marks requests as complete when
/// there's no async save in progress and no pending work in the queue.
#[cfg(not(target_family = "wasm"))]
pub(crate) fn notify_persistence_complete(
  persistence: Option<ResMut<PersistenceControl>>,
  saving: Res<SavingChunks>,
  tasks: Res<PersistenceTasks>,
  mut complete_messages: MessageWriter<PersistenceComplete>,
) {
  let Some(mut persistence) = persistence else {
    return;
  };

  // Don't complete requests if there's an async save in progress
  // or pending work in the queue
  if saving.is_busy() || has_pending_work(&tasks) {
    return;
  }

  for request in persistence.pending_requests.drain(..) {
    request.completed.store(true, Ordering::Release);
    complete_messages.write(PersistenceComplete {
      request_id: request.id,
      success: true,
      error: None,
    });
  }
}

/// System: Notifies pending save requests that they have completed (WASM
/// version).
///
/// On WASM, we don't have PersistenceControl, so this is a no-op for now.
/// TODO: Track WASM persistence requests separately.
#[cfg(target_family = "wasm")]
pub(crate) fn notify_persistence_complete(saving: Res<SavingChunks>, tasks: Res<PersistenceTasks>) {
  // On WASM, we just check if saves are complete
  if saving.is_busy() || has_pending_work(&tasks) {
    return;
  }
  // TODO: Implement WASM persistence request tracking
}

// ===== Async Save Systems =====
//
// These systems handle non-blocking batch saves using AsyncComputeTaskPool.
// The flow is:
// 1. `dispatch_save_task` - spawns async task when saves are queued
// 2. `poll_save_task` - checks task completion, merges results back

use crate::persistence::tasks::{LoadingChunks, SavingChunks};

/// System: Dispatches a batch save task when saves are queued.
///
/// Only one save task runs at a time to prevent write conflicts.
/// Runs in PostSimulation after chunks are queued.
///
/// Both native and WASM use IoDispatcher to send commands to the worker.
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
pub(crate) fn dispatch_save_task(
  mut saving: ResMut<SavingChunks>,
  mut tasks: ResMut<PersistenceTasks>,
  io_dispatcher: Option<Res<IoDispatcher>>,
) {
  // Don't dispatch if already saving or nothing to save
  if saving.is_busy() || !has_pending_work(&tasks) {
    return;
  }

  let Some(io_dispatcher) = io_dispatcher else {
    discard_queued_operations(&mut tasks);
    return;
  };

  if !io_dispatcher.is_ready() {
    discard_queued_operations(&mut tasks);
    return;
  }

  // Send WriteChunk commands for each chunk
  for task in tasks.save_queue.drain(..) {
    io_dispatcher.send(crate::persistence::IoCommand::WriteChunk {
      chunk_pos: bevy::math::IVec2::new(task.pos.x, task.pos.y),
      data: task.data,
    });
  }

  // Send SaveBody commands for each body
  for task in tasks.body_save_queue.drain(..) {
    let mut buf = Vec::new();
    if let Err(e) = task.record.write_to(&mut buf) {
      warn!("Failed to serialize body {}: {}", task.record.stable_id, e);
      continue;
    }
    io_dispatcher.send(crate::persistence::IoCommand::SaveBody {
      record_data: buf,
      stable_id: task.record.stable_id,
    });
  }

  // Send RemoveBody commands
  for task in tasks.body_remove_queue.drain(..) {
    io_dispatcher.send(crate::persistence::IoCommand::RemoveBody {
      stable_id: task.stable_id,
    });
  }

  // Send Flush to persist to disk
  io_dispatcher.send(crate::persistence::IoCommand::Flush);

  saving.busy = true;
}

/// System: Legacy poll for save tasks.
///
/// With the unified async architecture, saves now go through IoDispatcher
/// and completion is handled by poll_io_results. This system is kept for
/// backward compatibility with any legacy code that spawns tasks directly.
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
pub(crate) fn poll_save_task(
  #[cfg(not(target_family = "wasm"))] mut saving: ResMut<SavingChunks>,
  #[cfg(not(target_family = "wasm"))] rendering: Option<
    Res<crate::world::plugin::RenderingEnabled>,
  >,
) {
  #[cfg(not(target_family = "wasm"))]
  {
    // Legacy task support - if any code still spawns tasks directly
    let Some(ref mut task) = saving.task else {
      return;
    };

    let block_all = rendering.is_none();

    // Check if task is complete (or block if in test mode)
    if !block_all && !task.is_finished() {
      return;
    }

    // Get result
    let result = bevy::tasks::block_on(task);
    saving.task = None;
    saving.busy = false;

    // Log results (no longer need to merge since IoDispatcher handles this)
    for error in &result.errors {
      warn!("{}", error);
    }

    if result.chunks_saved > 0 || result.bodies_saved > 0 {
      info!(
        "Saved {} chunks, {} bodies, removed {} bodies",
        result.chunks_saved, result.bodies_saved, result.bodies_removed
      );
    }
  }

  // WASM: Save completion is handled by poll_io_results
  #[cfg(target_family = "wasm")]
  {}
}

/// System: Dispatches async load tasks for chunks entering the streaming
/// window.
///
/// When persistence is enabled, chunks in Loading state get load tasks
/// dispatched to check if they have persisted data.
///
/// - Native: Uses AsyncComputeTaskPool with direct file access
/// - WASM: Uses IoDispatcher to send LoadChunk commands to Web Worker
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
pub(crate) fn dispatch_chunk_loads(
  mut loading: ResMut<LoadingChunks>,
  io_dispatcher: Option<Res<IoDispatcher>>,
  worlds: Query<&PixelWorld>,
) {
  // Both native and WASM now use IoDispatcher for chunk loading
  let Some(io_dispatcher) = io_dispatcher else {
    return;
  };

  if !io_dispatcher.is_ready() {
    return;
  }

  for world in worlds.iter() {
    for (pos, slot_idx) in world.active_chunks() {
      let slot = world.slot(slot_idx);

      // Only dispatch for Loading state chunks not already being loaded
      if !slot.is_loading() || loading.pending.contains(&pos) {
        continue;
      }

      // Send LoadChunk command to worker
      io_dispatcher.send(crate::persistence::IoCommand::LoadChunk {
        chunk_pos: bevy::math::IVec2::new(pos.x, pos.y),
      });

      // Track that we're loading this chunk
      loading.pending.insert(pos);
    }
  }
}

/// System: Polls completed load tasks and transitions chunks to Seeding state.
///
/// When a load task completes, the loaded data is stored for the seeding system
/// to use, and the chunk transitions from Loading to Seeding.
///
/// When rendering is absent (no `RenderingEnabled` resource), all pending
/// tasks are block-waited to completion. This gives synchronous semantics
/// in test environments where frames advance faster than async tasks.
///
/// Note: On WASM, chunk loading is handled by `poll_io_results` instead.
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
pub(crate) fn poll_chunk_loads(
  #[cfg(not(target_family = "wasm"))] mut loading: ResMut<LoadingChunks>,
  #[cfg(not(target_family = "wasm"))] mut worlds: Query<&mut PixelWorld>,
  #[cfg(not(target_family = "wasm"))] mut loaded_data: ResMut<LoadedChunkDataStore>,
  #[cfg(not(target_family = "wasm"))] rendering: Option<
    Res<crate::world::plugin::RenderingEnabled>,
  >,
) {
  // Legacy native task polling (for tasks still using AsyncComputeTaskPool).
  // Most chunk loading now goes through IoDispatcher and poll_io_results.
  #[cfg(not(target_family = "wasm"))]
  {
    // If there are no tasks, skip processing
    if loading.tasks.is_empty() {
      return;
    }

    let block_all = rendering.is_none();

    // Collect positions that finished loading
    let mut completed_positions = Vec::new();

    loading.tasks.retain(|pos, task| {
      if !block_all && !task.is_finished() {
        return true; // Keep polling
      }

      let result = bevy::tasks::block_on(task);
      completed_positions.push(*pos);

      // Log errors
      if let Some(ref error) = result.error {
        warn!("{}", error);
      }

      // Store loaded data first (before iterating worlds)
      if let Some(data) = result.data {
        loaded_data.store.insert(*pos, data);
      }

      // Find the world and slot for this position
      for mut world in worlds.iter_mut() {
        if let Some(slot_idx) = world.get_slot_index(*pos) {
          let slot = world.slot_mut(slot_idx);

          // Transition to Seeding state
          if slot.is_loading() {
            slot.lifecycle = crate::world::slot::ChunkLifecycle::Seeding;
          }
        }
      }

      false // Remove completed task
    });

    // Remove completed positions from pending set
    for pos in completed_positions {
      loading.pending.remove(&pos);
    }
  }

  // WASM: Chunk loading is handled by poll_io_results
  #[cfg(target_family = "wasm")]
  {}
}

/// Resource storing loaded chunk data waiting to be applied during seeding.
#[derive(Resource, Default)]
pub struct LoadedChunkDataStore {
  /// Map from chunk position to loaded chunk data.
  pub store: std::collections::HashMap<crate::coords::ChunkPos, crate::persistence::LoadedChunk>,
  /// Map from chunk position to loaded body data.
  pub bodies: std::collections::HashMap<
    crate::coords::ChunkPos,
    Vec<crate::persistence::io_worker::BodyLoadData>,
  >,
}

impl LoadedChunkDataStore {
  /// Takes loaded chunk data for a position, if any.
  pub fn take(&mut self, pos: crate::coords::ChunkPos) -> Option<crate::persistence::LoadedChunk> {
    self.store.remove(&pos)
  }

  /// Takes loaded body data for a position, if any.
  pub fn take_bodies(
    &mut self,
    pos: crate::coords::ChunkPos,
  ) -> Vec<crate::persistence::io_worker::BodyLoadData> {
    self.bodies.remove(&pos).unwrap_or_default()
  }
}

// ===== I/O Worker Integration =====
//
// On WASM, we use a dedicated Web Worker for OPFS operations because
// `createSyncAccessHandle()` only works in Web Workers, not the main thread.
// Bevy's AsyncComputeTaskPool runs on the main thread in WASM.

use crate::persistence::io_worker::{IoDispatcher, IoResult};

/// System: Polls the I/O worker for results and handles them.
///
/// This system handles:
/// - Initialization results (sets up PersistenceControl)
/// - Chunk load results (stores data for seeding)
/// - Write completion results (updates tracking)
/// - Flush completion
/// - Errors
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
pub(crate) fn poll_io_results(
  mut commands: Commands,
  io_dispatcher: Option<Res<IoDispatcher>>,
  pending_init: Option<Res<crate::world::control::PendingPersistenceInit>>,
  mut loaded_data: ResMut<LoadedChunkDataStore>,
  mut worlds: Query<&mut PixelWorld>,
  mut loading: ResMut<LoadingChunks>,
  mut saving: ResMut<SavingChunks>,
) {
  let Some(io_dispatcher) = io_dispatcher else {
    return;
  };

  // Process all available results
  while let Some(result) = io_dispatcher.try_recv() {
    match result {
      IoResult::Initialized {
        chunk_count,
        body_count,
        world_seed,
      } => {
        info!(
          "I/O Worker initialized: {} chunks, {} bodies, seed {}",
          chunk_count, body_count, world_seed
        );
        io_dispatcher.set_ready(true);
        io_dispatcher.set_world_seed(world_seed);
        io_dispatcher.set_init_counts(chunk_count, body_count);

        // Create PersistenceControl now that worker is ready
        if let Some(ref pending) = pending_init {
          commands.insert_resource(PersistenceControl::with_path_only(pending.path.clone()));
          commands.remove_resource::<crate::world::control::PendingPersistenceInit>();
        }
      }
      IoResult::ChunkLoaded {
        chunk_pos,
        data,
        bodies,
      } => {
        let pos = crate::coords::ChunkPos::new(chunk_pos.x, chunk_pos.y);

        // Remove from pending set
        loading.pending.remove(&pos);

        // Store loaded chunk data if present
        if let Some(chunk_data) = data {
          let storage_type = match chunk_data.storage_type {
            0 => crate::persistence::format::StorageType::Empty,
            1 => crate::persistence::format::StorageType::Delta,
            _ => crate::persistence::format::StorageType::Full,
          };
          loaded_data.store.insert(
            pos,
            crate::persistence::LoadedChunk {
              storage_type,
              data: chunk_data.data,
              pos,
              seeder_needed: chunk_data.seeder_needed,
            },
          );
        }

        // Store loaded body data for later spawning (when chunk finishes seeding)
        if !bodies.is_empty() {
          loaded_data.bodies.insert(pos, bodies);
        }

        // Transition chunk to Seeding state
        for mut world in worlds.iter_mut() {
          if let Some(slot_idx) = world.get_slot_index(pos) {
            let slot = world.slot_mut(slot_idx);
            if slot.is_loading() {
              slot.lifecycle = crate::world::slot::ChunkLifecycle::Seeding;
            }
          }
        }
      }
      IoResult::WriteComplete { chunk_pos: _ } => {
        // Write completed, nothing to do here
        // The flush will happen separately
      }
      IoResult::BodySaveComplete { stable_id: _ } => {
        // Body save completed
      }
      IoResult::BodyRemoveComplete { stable_id: _ } => {
        // Body removal completed
      }
      IoResult::FlushComplete => {
        info!("I/O Worker flush complete");
        // Reset busy flag so new saves can be dispatched
        saving.busy = false;
      }
      IoResult::Error { message } => {
        warn!("I/O Worker error: {}", message);
      }
    }
  }
}
