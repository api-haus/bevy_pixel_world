//! Async persistence tasks for non-blocking I/O.
//!
//! Provides resources and types for managing async chunk load/save operations
//! via Bevy's AsyncComputeTaskPool.

#[cfg(not(target_family = "wasm"))]
use std::collections::HashMap;
use std::io;
use std::sync::Arc;

use bevy::prelude::*;
#[cfg(not(target_family = "wasm"))]
use bevy::tasks::Task;

use super::backend::StorageFile;
use super::format::{PageTableEntry, StorageType};
use super::index::{ChunkIndex, PixelBodyIndex, PixelBodyIndexEntry};
use super::{BodyRemoveTask, BodySaveTask, LoadedChunk, SaveTask};
use crate::pixel_world::coords::ChunkPos;

/// Resource tracking in-flight chunk load tasks.
///
/// Each task reads a single chunk from disk asynchronously.
/// Multiple load tasks can run concurrently (read-only operations).
#[derive(Resource, Default)]
pub struct LoadingChunks {
  /// Positions currently being loaded.
  pub pending: std::collections::HashSet<ChunkPos>,
  /// Native-only: async tasks for in-flight loads.
  #[cfg(not(target_family = "wasm"))]
  pub(crate) tasks: HashMap<ChunkPos, Task<LoadResult>>,
}

impl LoadingChunks {
  /// Returns true if there are no in-flight load tasks.
  pub fn is_empty(&self) -> bool {
    self.pending.is_empty()
  }

  /// Returns the number of in-flight load tasks.
  pub fn len(&self) -> usize {
    self.pending.len()
  }
}

/// Resource tracking the in-flight batch save task.
///
/// Only one save task runs at a time to prevent write conflicts.
#[derive(Resource, Default)]
pub struct SavingChunks {
  /// Whether a save is currently in progress.
  pub(crate) busy: bool,
  /// Native-only: async task for the current save.
  #[cfg(not(target_family = "wasm"))]
  pub(crate) task: Option<Task<SaveResult>>,
}

impl SavingChunks {
  /// Returns true if a save task is in progress.
  pub fn is_busy(&self) -> bool {
    self.busy
  }
}

/// Result of loading a single chunk from disk.
#[derive(Debug)]
pub struct LoadResult {
  /// The chunk position that was loaded.
  pub pos: ChunkPos,
  /// The loaded chunk data, or None if not in save file.
  pub data: Option<LoadedChunk>,
  /// Error message if load failed.
  pub error: Option<String>,
}

impl LoadResult {
  /// Creates a successful load result with data.
  pub fn success(pos: ChunkPos, data: LoadedChunk) -> Self {
    Self {
      pos,
      data: Some(data),
      error: None,
    }
  }

  /// Creates a result indicating the chunk is not persisted.
  pub fn not_found(pos: ChunkPos) -> Self {
    Self {
      pos,
      data: None,
      error: None,
    }
  }

  /// Creates a result indicating a load error.
  pub fn error(pos: ChunkPos, error: impl Into<String>) -> Self {
    Self {
      pos,
      data: None,
      error: Some(error.into()),
    }
  }
}

/// Result of a batch save operation.
pub struct SaveResult {
  /// Updated chunk index after saves.
  pub chunk_index: ChunkIndex,
  /// Updated pixel body index after saves.
  pub body_index: PixelBodyIndex,
  /// New data write position after all writes.
  pub data_write_pos: u64,
  /// Number of chunks saved.
  pub chunks_saved: usize,
  /// Number of bodies saved.
  pub bodies_saved: usize,
  /// Number of bodies removed.
  pub bodies_removed: usize,
  /// Error messages from failed operations.
  pub errors: Vec<String>,
}

/// Input data for a batch save operation.
pub struct SaveBatchInput {
  /// Chunks to save.
  pub chunks: Vec<SaveTask>,
  /// Bodies to save.
  pub bodies: Vec<BodySaveTask>,
  /// Bodies to remove.
  pub removals: Vec<BodyRemoveTask>,
  /// Snapshot of the chunk index at dispatch time.
  pub chunk_index: ChunkIndex,
  /// Snapshot of the body index at dispatch time.
  pub body_index: PixelBodyIndex,
  /// Current data write position.
  pub data_write_pos: u64,
}

/// Loads a single chunk from the save file asynchronously.
///
/// This function is designed to be called from within an async task context.
pub async fn load_chunk_async(
  file: &dyn StorageFile,
  index: &ChunkIndex,
  pos: ChunkPos,
) -> LoadResult {
  let Some(entry) = index.get(pos) else {
    return LoadResult::not_found(pos);
  };

  // Read compressed data
  let mut data = vec![0u8; entry.data_size as usize];
  if let Err(e) = file.read_at(entry.data_offset, &mut data).await {
    return LoadResult::error(pos, format!("Failed to read chunk {:?}: {}", pos, e));
  }

  LoadResult::success(
    pos,
    LoadedChunk {
      storage_type: entry.storage_type,
      data,
      pos,
      seeder_needed: entry.storage_type == StorageType::Delta,
    },
  )
}

/// Saves a batch of chunks and bodies to the save file asynchronously.
///
/// This function is designed to be called from within an async task context.
/// It writes all data sequentially to prevent concurrent write issues.
pub async fn save_batch_async(file: Arc<dyn StorageFile>, mut input: SaveBatchInput) -> SaveResult {
  let mut errors = Vec::new();
  let mut chunks_saved = 0;
  let mut bodies_saved = 0;
  let mut bodies_removed = 0;

  // Save chunks
  for task in input.chunks {
    match save_single_chunk(
      &*file,
      &mut input.chunk_index,
      &mut input.data_write_pos,
      task,
    )
    .await
    {
      Ok(()) => chunks_saved += 1,
      Err(e) => errors.push(e),
    }
  }

  // Save bodies
  for task in input.bodies {
    match save_single_body(
      &*file,
      &mut input.body_index,
      &mut input.data_write_pos,
      task,
    )
    .await
    {
      Ok(()) => bodies_saved += 1,
      Err(e) => errors.push(e),
    }
  }

  // Remove bodies
  for task in input.removals {
    if input.body_index.remove(task.stable_id).is_some() {
      bodies_removed += 1;
    }
  }

  SaveResult {
    chunk_index: input.chunk_index,
    body_index: input.body_index,
    data_write_pos: input.data_write_pos,
    chunks_saved,
    bodies_saved,
    bodies_removed,
    errors,
  }
}

/// Saves a single chunk to the file.
async fn save_single_chunk(
  file: &dyn StorageFile,
  index: &mut ChunkIndex,
  write_pos: &mut u64,
  task: SaveTask,
) -> Result<(), String> {
  // Write size prefix + data
  let size_bytes = (task.data.len() as u32).to_le_bytes();
  let mut write_buf = Vec::with_capacity(4 + task.data.len());
  write_buf.extend_from_slice(&size_bytes);
  write_buf.extend_from_slice(&task.data);

  file
    .write_at(*write_pos, &write_buf)
    .await
    .map_err(|e| format!("Failed to save chunk {:?}: {}", task.pos, e))?;

  // Create page table entry
  let entry = PageTableEntry::new(
    task.pos,
    *write_pos + 4, // Skip size prefix
    task.data.len() as u32,
    task.storage_type,
  );

  // Update state
  index.insert(entry);
  *write_pos += 4 + task.data.len() as u64;

  Ok(())
}

/// Saves a single pixel body to the file.
async fn save_single_body(
  file: &dyn StorageFile,
  index: &mut PixelBodyIndex,
  write_pos: &mut u64,
  task: BodySaveTask,
) -> Result<(), String> {
  // Serialize to buffer
  let mut buf = Vec::new();
  task
    .record
    .write_to(&mut buf)
    .map_err(|e| format!("Failed to serialize body {}: {}", task.record.stable_id, e))?;

  // Write to file
  file
    .write_at(*write_pos, &buf)
    .await
    .map_err(|e| format!("Failed to write body {}: {}", task.record.stable_id, e))?;

  // Create index entry
  let entry = PixelBodyIndexEntry {
    stable_id: task.record.stable_id,
    data_offset: *write_pos,
    data_size: buf.len() as u32,
    chunk_pos: task.record.chunk_pos(),
  };

  // Update state
  index.insert(entry);
  *write_pos += buf.len() as u64;

  Ok(())
}

/// Flushes metadata (header, page table, entity section) to the save file.
///
/// This should be called after batch saves complete to persist the indices.
pub async fn flush_metadata_async(
  file: &dyn StorageFile,
  header: &mut super::format::Header,
  chunk_index: &ChunkIndex,
  body_index: &PixelBodyIndex,
  data_write_pos: u64,
) -> io::Result<()> {
  // Update header timestamps
  #[cfg(not(target_family = "wasm"))]
  {
    header.modified_time = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .map(|d| d.as_secs())
      .unwrap_or(0);
  }
  #[cfg(target_family = "wasm")]
  {
    header.modified_time = (js_sys::Date::now() / 1000.0) as u64;
  }

  header.chunk_count = chunk_index.len() as u32;
  header.page_table_size = chunk_index.serialized_size() as u32;
  header.data_region_ptr = data_write_pos;

  // Serialize page table
  let mut page_table_buf = Vec::new();
  chunk_index.write_to(&mut page_table_buf)?;
  file
    .write_at(data_write_pos, &page_table_buf)
    .await
    .map_err(io::Error::from)?;

  // Entity section goes after page table
  let entity_section_start = data_write_pos + chunk_index.serialized_size() as u64;
  header.entity_section_ptr = if body_index.is_empty() {
    0
  } else {
    entity_section_start
  };

  // Write entity section if we have bodies
  if !body_index.is_empty() {
    let entity_header = super::format::EntitySectionHeader {
      entity_count: body_index.len() as u32,
      _reserved: 0,
    };

    let mut entity_buf = Vec::new();
    entity_header.write_to(&mut entity_buf)?;
    body_index.write_to(&mut entity_buf)?;

    file
      .write_at(entity_section_start, &entity_buf)
      .await
      .map_err(io::Error::from)?;
  }

  // Write updated header
  let mut header_buf = Vec::new();
  header.write_to(&mut header_buf)?;
  file
    .write_at(0, &header_buf)
    .await
    .map_err(io::Error::from)?;

  // Sync to disk
  file.sync().await.map_err(io::Error::from)?;

  Ok(())
}
