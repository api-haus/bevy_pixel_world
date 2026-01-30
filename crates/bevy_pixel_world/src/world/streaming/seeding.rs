//! Chunk seeding systems.
//!
//! Handles asynchronous chunk generation through the seeder trait.

use std::collections::HashSet;

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};

use super::SeededChunks;
use crate::coords::{CHUNK_SIZE, ChunkPos};
use crate::debug_shim;
use crate::persistence::LoadedChunk;
use crate::persistence::tasks::LoadingChunks;
use crate::primitives::Chunk;
use crate::world::PixelWorld;
use crate::world::SlotIndex;
use crate::world::control::{ReloadAllChunks, ReseedAllChunks};
use crate::world::persistence_systems::LoadedChunkDataStore;
use crate::world::slot::ChunkLifecycle;

/// Resource holding async seeding tasks.
#[derive(Resource, Default)]
pub(crate) struct SeedingTasks {
  pub(super) tasks: Vec<SeedingTask>,
}

impl SeedingTasks {
  /// Returns the number of in-flight seeding tasks.
  pub fn len(&self) -> usize {
    self.tasks.len()
  }

  /// Returns true if there are no in-flight seeding tasks.
  pub fn is_empty(&self) -> bool {
    self.tasks.is_empty()
  }
}

/// An in-flight seeding task.
pub(super) struct SeedingTask {
  /// Which PixelWorld entity.
  pub world_entity: Entity,
  /// Which slot is being seeded.
  pub slot_index: SlotIndex,
  /// The chunk position being seeded.
  pub pos: ChunkPos,
  /// The async task returning a seeded chunk.
  pub task: Task<Chunk>,
}

/// Maximum number of concurrent seeding tasks.
const MAX_SEEDING_TASKS: usize = 2;

/// Creates and seeds a new chunk with pre-loaded persistence data.
///
/// Applies loaded data directly instead of relying on the seeder's
/// `seed_with_loaded` method. This ensures loaded data is used regardless
/// of whether the seeder is wrapped with `PersistenceSeeder`.
pub(crate) fn seed_chunk_with_loaded(
  seeder: &(dyn crate::seeding::ChunkSeeder + Send + Sync),
  pos: ChunkPos,
  loaded: Option<LoadedChunk>,
) -> Chunk {
  let mut chunk = Chunk::new(CHUNK_SIZE, CHUNK_SIZE);
  chunk.set_pos(pos);

  if let Some(loaded_chunk) = loaded {
    // Apply loaded data directly
    if loaded_chunk.seeder_needed {
      // Delta encoding - seed first, then apply delta
      seeder.seed(pos, &mut chunk);
    }
    if let Err(e) = loaded_chunk.apply_to(&mut chunk) {
      warn!(
        "Failed to apply saved chunk at {:?}: {}. Regenerating.",
        pos, e
      );
      seeder.seed(pos, &mut chunk);
    } else {
      chunk.from_persistence = true;
    }
  } else {
    // No loaded data, just seed procedurally
    seeder.seed(pos, &mut chunk);
  }

  chunk
}

/// Merges seeded pixels into existing chunk, preserving PIXEL_BODY pixels.
///
/// When seeding completes asynchronously, pixel bodies may have already
/// blitted to the chunk. We must not overwrite those pixels or they'll
/// be detected as destroyed.
pub(crate) fn merge_seeded_pixels(
  existing: &mut crate::pixel::PixelSurface,
  seeded: &crate::pixel::PixelSurface,
) {
  use crate::pixel::PixelFlags;

  let existing_slice = existing.as_slice_mut();
  let seeded_slice = seeded.as_slice();

  for (existing_pixel, seeded_pixel) in existing_slice.iter_mut().zip(seeded_slice.iter()) {
    // Only overwrite if existing pixel doesn't have PIXEL_BODY flag
    if !existing_pixel.flags.contains(PixelFlags::PIXEL_BODY) {
      *existing_pixel = *seeded_pixel;
    }
  }
}

/// Collects in-flight seeding task count and slot indices for a world entity.
fn collect_in_flight_tasks(
  tasks: &[SeedingTask],
  world_entity: Entity,
) -> (usize, HashSet<SlotIndex>) {
  let mut count = 0;
  let mut slots = HashSet::new();
  for task in tasks {
    if task.world_entity == world_entity {
      count += 1;
      slots.insert(task.slot_index);
    }
  }
  (count, slots)
}

/// Spawns an async seeding task for a chunk with optional pre-loaded data.
fn spawn_seeding_task(
  seeding_tasks: &mut SeedingTasks,
  task_pool: &AsyncComputeTaskPool,
  world_entity: Entity,
  world: &PixelWorld,
  pos: ChunkPos,
  slot_idx: SlotIndex,
  loaded: Option<LoadedChunk>,
) {
  let seeder = world.seeder().clone();
  let task = task_pool.spawn(async move { seed_chunk_with_loaded(seeder.as_ref(), pos, loaded) });

  seeding_tasks.tasks.push(SeedingTask {
    world_entity,
    slot_index: slot_idx,
    pos,
    task,
  });
}

/// System: Dispatches async seeding tasks for chunks in Seeding state.
///
/// Chunks must be in Seeding state (not Loading or Active) to have tasks
/// dispatched. Pre-loaded persistence data is passed to the seeder if
/// available.
///
/// When rendering is absent, all seeding chunks are dispatched at once
/// (no task limit), so `poll_seeding_tasks` can block-complete them in
/// the same frame.
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
pub(crate) fn dispatch_seeding(
  mut seeding_tasks: ResMut<SeedingTasks>,
  mut worlds: Query<(Entity, &mut PixelWorld)>,
  mut loaded_data: ResMut<LoadedChunkDataStore>,
  rendering: Option<Res<crate::world::plugin::RenderingEnabled>>,
  async_behavior: Option<Res<crate::world::plugin::AsyncTaskBehavior>>,
) {
  let task_pool = AsyncComputeTaskPool::get();
  let block_all = crate::world::plugin::should_block_tasks(rendering, async_behavior);
  let max_tasks = if block_all {
    usize::MAX
  } else {
    MAX_SEEDING_TASKS
  };

  for (world_entity, world) in worlds.iter_mut() {
    let (mut in_flight, in_flight_slots) =
      collect_in_flight_tasks(&seeding_tasks.tasks, world_entity);

    if in_flight >= max_tasks {
      continue;
    }

    for (pos, slot_idx) in world.active_chunks() {
      if in_flight_slots.contains(&slot_idx) {
        continue;
      }

      let slot = world.slot(slot_idx);
      // Only dispatch for chunks in Seeding state (not Loading or Active)
      if !slot.is_seeding() {
        continue;
      }

      // Take any pre-loaded data for this chunk
      let loaded = loaded_data.take(pos);

      spawn_seeding_task(
        &mut seeding_tasks,
        task_pool,
        world_entity,
        &world,
        pos,
        slot_idx,
        loaded,
      );

      in_flight += 1;
      if in_flight >= max_tasks {
        break;
      }
    }
  }
}

/// System: Polls completed seeding tasks and swaps in seeded chunks.
///
/// When rendering is absent (no `RenderingEnabled` resource), all pending
/// tasks are block-waited to completion. This gives synchronous semantics
/// in test environments where frames advance faster than async tasks.
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
pub(crate) fn poll_seeding_tasks(
  mut seeding_tasks: ResMut<SeedingTasks>,
  mut worlds: Query<&mut PixelWorld>,
  mut seeded_chunks: ResMut<SeededChunks>,
  gizmos: debug_shim::GizmosParam,
  rendering: Option<Res<crate::world::plugin::RenderingEnabled>>,
  async_behavior: Option<Res<crate::world::plugin::AsyncTaskBehavior>>,
) {
  let debug_gizmos = gizmos.get();
  let block_all = crate::world::plugin::should_block_tasks(rendering, async_behavior);

  seeding_tasks.tasks.retain_mut(|task| {
    if !block_all && !task.task.is_finished() {
      return true; // keep pending tasks
    }

    let seeded_chunk = bevy::tasks::block_on(&mut task.task);

    if let Ok(mut world) = worlds.get_mut(task.world_entity)
      // Slot may have been recycled if camera moved while task was in flight.
      // Both checks are needed: position mapping and slot index must match.
      && let Some(current_idx) = world.get_slot_index(task.pos)
      && current_idx == task.slot_index
    {
      let slot = world.slot_mut(task.slot_index);
      // Merge seeded pixels, preserving any PIXEL_BODY pixels that were
      // blitted before seeding completed.
      merge_seeded_pixels(&mut slot.chunk.pixels, &seeded_chunk.pixels);
      slot.chunk.set_all_dirty_rects_full();
      slot.lifecycle = ChunkLifecycle::Active;
      slot.dirty = true;

      // If loaded from disk, mark as persisted (no need to save again)
      if seeded_chunk.from_persistence {
        slot.persisted = true;
      }

      // Track that this chunk just finished seeding
      seeded_chunks.positions.push(task.pos);

      debug_shim::emit_chunk(debug_gizmos, task.pos);
    }

    false // remove completed task
  });
}

/// System: Handles reseed requests by transitioning Active chunks to Seeding.
///
/// When `ReseedAllChunks` is sent, all active chunks regenerate with the
/// current noise profile. Any cached persistence data is cleared first.
///
/// Use this for level editor mode when noise profiles change.
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
pub(crate) fn handle_reseed_request(
  mut events: bevy::ecs::message::MessageReader<ReseedAllChunks>,
  mut worlds: Query<&mut PixelWorld>,
  mut loaded_data: ResMut<LoadedChunkDataStore>,
) {
  // Only process if there are reseed requests
  if events.is_empty() {
    return;
  }

  // Consume all events (we only need to reseed once even if multiple events)
  for _ in events.read() {}

  // Clear any cached persistence data
  loaded_data.store.clear();
  loaded_data.bodies.clear();

  // Transition Active chunks back to Seeding
  let mut count = 0;
  for mut world in &mut worlds {
    for (_pos, slot_idx) in world.active_chunks().collect::<Vec<_>>() {
      let slot = world.slot_mut(slot_idx);
      if slot.lifecycle == ChunkLifecycle::Active {
        slot.lifecycle = ChunkLifecycle::Seeding;
        slot.chunk.from_persistence = false;
        count += 1;
      }
    }
  }

  if count > 0 {
    info!("Re-seeding {} chunks", count);
  }
}

/// System: Handles reload requests by transitioning Active chunks to Loading.
///
/// When `ReloadAllChunks` is sent, all active chunks reload from disk,
/// discarding any unsaved in-memory changes.
///
/// Use this to revert to the last saved state.
#[cfg_attr(feature = "tracy", tracing::instrument(skip_all))]
pub(crate) fn handle_reload_request(
  mut events: bevy::ecs::message::MessageReader<ReloadAllChunks>,
  mut worlds: Query<&mut PixelWorld>,
  mut loaded_data: ResMut<LoadedChunkDataStore>,
  mut loading: ResMut<LoadingChunks>,
) {
  if events.is_empty() {
    return;
  }

  // Consume all events
  for _ in events.read() {}

  // Clear caches so chunks re-request from disk
  loaded_data.store.clear();
  loaded_data.bodies.clear();
  loading.pending.clear();

  // Transition Active → Loading (will be re-dispatched by dispatch_chunk_loads)
  let mut count = 0;
  for mut world in &mut worlds {
    for (_pos, slot_idx) in world.active_chunks().collect::<Vec<_>>() {
      let slot = world.slot_mut(slot_idx);
      if slot.lifecycle == ChunkLifecycle::Active {
        slot.lifecycle = ChunkLifecycle::Loading;
        count += 1;
      }
    }
  }

  if count > 0 {
    info!("Reloading {} chunks from disk", count);
  }
}
