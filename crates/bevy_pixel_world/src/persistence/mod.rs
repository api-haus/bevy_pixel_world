//! Chunk persistence - save/load modified chunks to disk.
//!
//! This module provides disk storage for modified chunks, allowing player
//! modifications to persist across sessions while procedurally regenerating
//! unmodified areas.
//!
//! See `docs/architecture/chunk-persistence.md` for the full specification.

pub mod backend;
pub mod compression;
pub mod format;
pub mod index;
pub mod io_worker;
#[cfg(not(target_family = "wasm"))]
pub mod native;
#[cfg(target_family = "wasm")]
pub mod opfs;
pub mod pixel_body;
pub mod tasks;

use std::io::{self, Cursor};
use std::path::PathBuf;
use std::sync::Arc;

use backend::{StorageFile, StorageFs};
use bevy::prelude::*;
use compression::{
  apply_delta, compute_delta, decode_delta, decode_full, encode_delta, encode_full,
  should_use_delta,
};
use format::{EntitySectionHeader, Header, HeaderError, PageTableEntry, StorageType};
use index::{ChunkIndex, PixelBodyIndex, PixelBodyIndexEntry};
pub use io_worker::{IoCommand, IoDispatcher, IoResult};
// Re-export backend implementations
#[cfg(not(target_family = "wasm"))]
pub use native::NativePersistence;
#[cfg(target_family = "wasm")]
pub use opfs::WasmPersistence;
pub use pixel_body::{PixelBodyReadError, PixelBodyRecord};

use crate::coords::ChunkPos;
use crate::primitives::Chunk;
use crate::seeding::ChunkSeeder;

/// Default application name for save directory.
pub const DEFAULT_APP_NAME: &str = "pixel_world";

/// Returns the default save directory for the given app name.
///
/// Uses OS-standard data directories:
/// - Linux: `~/.local/share/<app_name>/saves/`
/// - Windows: `%APPDATA%/<app_name>/saves/`
/// - macOS: `~/Library/Application Support/<app_name>/saves/`
#[cfg(feature = "native")]
pub fn default_save_dir(app_name: &str) -> PathBuf {
  dirs::data_dir()
    .unwrap_or_else(|| PathBuf::from("."))
    .join(app_name)
    .join("saves")
}

/// WASM: Returns a placeholder path (persistence not yet supported).
#[cfg(not(feature = "native"))]
pub fn default_save_dir(_app_name: &str) -> PathBuf {
  PathBuf::from(".")
}

/// Polls a future that is expected to be immediately ready.
///
/// All native backend futures resolve on first poll. This helper avoids
/// pulling in a full async runtime for what is synchronous I/O.
#[cfg(not(target_family = "wasm"))]
pub(crate) fn block_on<T>(
  fut: std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + '_>>,
) -> T {
  use std::task::{Context, Poll, Wake, Waker};

  struct NoopWaker;
  impl Wake for NoopWaker {
    fn wake(self: Arc<Self>) {}
  }

  let waker = Waker::from(Arc::new(NoopWaker));
  let mut cx = Context::from_waker(&waker);
  let mut fut = fut;

  // For native backend, futures resolve on first poll.
  // Loop handles edge cases where a backend needs multiple polls.
  loop {
    match fut.as_mut().poll(&mut cx) {
      Poll::Ready(val) => return val,
      Poll::Pending => {
        // Yield and try again — should not happen with native backend
        std::thread::yield_now();
      }
    }
  }
}

/// WASM version of block_on (no Send requirement, no thread yielding).
#[cfg(target_family = "wasm")]
pub(crate) fn block_on<T>(fut: std::pin::Pin<Box<dyn std::future::Future<Output = T> + '_>>) -> T {
  use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

  // Minimal no-op waker for WASM
  const VTABLE: RawWakerVTable = RawWakerVTable::new(
    |_| RawWaker::new(std::ptr::null(), &VTABLE),
    |_| {},
    |_| {},
    |_| {},
  );
  let raw_waker = RawWaker::new(std::ptr::null(), &VTABLE);
  let waker = unsafe { Waker::from_raw(raw_waker) };
  let mut cx = Context::from_waker(&waker);
  let mut fut = fut;

  loop {
    match fut.as_mut().poll(&mut cx) {
      Poll::Ready(val) => return val,
      Poll::Pending => {
        // On WASM, if a future pends it won't resolve without an async runtime
        panic!("block_on: future returned Pending on WASM (requires async runtime)");
      }
    }
  }
}

/// World save file handle with runtime index.
///
/// Holds the open save file and in-memory index for O(1) chunk lookups.
/// I/O is performed through a [`StorageFile`] trait object, allowing
/// different backends (native filesystem, OPFS, etc.).
pub struct WorldSave {
  /// The file name (e.g., "world.save").
  pub(crate) name: String,
  /// Open file handle via the storage backend.
  /// Arc-wrapped for sharing with async tasks.
  pub(crate) file: Arc<dyn StorageFile>,
  /// File header.
  pub(crate) header: Header,
  /// Runtime index mapping positions to page table entries.
  pub(crate) index: ChunkIndex,
  /// Runtime index for pixel bodies.
  pub(crate) body_index: PixelBodyIndex,
  /// Current write position in data region (for append).
  pub(crate) data_write_pos: u64,
  /// Whether the save has been modified since last flush.
  pub(crate) dirty: bool,
}

impl WorldSave {
  /// Creates a new save file with the given name via a storage backend.
  pub fn create(fs: &dyn StorageFs, name: &str, world_seed: u64) -> io::Result<Self> {
    let file = block_on(fs.create(name)).map_err(io::Error::from)?;

    let header = Header::new(world_seed);
    let data_write_pos = Header::SIZE as u64;

    // Serialize and write initial header
    let mut buf = Vec::new();
    header.write_to(&mut buf)?;
    block_on(file.write_at(0, &buf)).map_err(io::Error::from)?;
    block_on(file.sync()).map_err(io::Error::from)?;

    Ok(Self {
      name: name.to_string(),
      file: Arc::from(file),
      header,
      index: ChunkIndex::new(),
      body_index: PixelBodyIndex::new(),
      data_write_pos,
      dirty: false,
    })
  }

  /// Opens an existing save file via a storage backend.
  pub fn open(fs: &dyn StorageFs, name: &str) -> Result<Self, OpenError> {
    let file = block_on(fs.open(name)).map_err(|e| OpenError::Io(io::Error::from(e)))?;

    // Read header
    let mut header_buf = [0u8; Header::SIZE];
    block_on(file.read_at(0, &mut header_buf)).map_err(|e| OpenError::Io(io::Error::from(e)))?;
    let header = Header::read_from(&mut Cursor::new(&header_buf))?;
    header.validate()?;

    // Read page table
    let page_table_size = header.chunk_count as usize * PageTableEntry::SIZE;
    let mut page_table_buf = vec![0u8; page_table_size];
    block_on(file.read_at(header.data_region_ptr, &mut page_table_buf))
      .map_err(|e| OpenError::Io(io::Error::from(e)))?;
    let index = ChunkIndex::read_from(
      &mut Cursor::new(&page_table_buf),
      header.chunk_count as usize,
    )?;

    // Read entity section if present
    let body_index = if header.entity_section_ptr != 0 {
      let mut entity_header_buf = [0u8; EntitySectionHeader::SIZE];
      block_on(file.read_at(header.entity_section_ptr, &mut entity_header_buf))
        .map_err(|e| OpenError::Io(io::Error::from(e)))?;
      let entity_header = EntitySectionHeader::read_from(&mut Cursor::new(&entity_header_buf))?;

      let body_index_size = entity_header.entity_count as usize * PixelBodyIndexEntry::SIZE;
      let mut body_index_buf = vec![0u8; body_index_size];
      let body_data_offset = header.entity_section_ptr + EntitySectionHeader::SIZE as u64;
      block_on(file.read_at(body_data_offset, &mut body_index_buf))
        .map_err(|e| OpenError::Io(io::Error::from(e)))?;
      PixelBodyIndex::read_from(
        &mut Cursor::new(&body_index_buf),
        entity_header.entity_count as usize,
      )?
    } else {
      PixelBodyIndex::new()
    };

    let data_write_pos = header.data_region_ptr;

    Ok(Self {
      name: name.to_string(),
      file: Arc::from(file),
      header,
      index,
      body_index,
      data_write_pos,
      dirty: false,
    })
  }

  /// Opens an existing save file or creates a new one.
  pub fn open_or_create(
    fs: &dyn StorageFs,
    name: &str,
    world_seed: u64,
  ) -> Result<Self, OpenError> {
    let exists = block_on(fs.exists(name)).map_err(|e| OpenError::Io(io::Error::from(e)))?;
    if exists {
      Self::open(fs, name)
    } else {
      Ok(Self::create(fs, name, world_seed)?)
    }
  }

  /// Opens an existing save file or creates a new one asynchronously.
  ///
  /// This is the WASM-compatible version that uses `.await` instead of
  /// `block_on()`. Required because OPFS operations return actual async
  /// futures that cannot be polled to completion synchronously.
  #[cfg(target_family = "wasm")]
  pub async fn open_or_create_async(
    fs: &dyn StorageFs,
    name: &str,
    world_seed: u64,
  ) -> Result<Self, String> {
    let exists = fs
      .exists(name)
      .await
      .map_err(|e| format!("Failed to check file existence: {}", e))?;

    if exists {
      Self::open_async(fs, name).await
    } else {
      Self::create_async(fs, name, world_seed).await
    }
  }

  /// Creates a new save file asynchronously (WASM).
  #[cfg(target_family = "wasm")]
  async fn create_async(fs: &dyn StorageFs, name: &str, world_seed: u64) -> Result<Self, String> {
    let file = fs
      .create(name)
      .await
      .map_err(|e| format!("Failed to create file: {}", e))?;

    let header = Header::new(world_seed);
    let data_write_pos = Header::SIZE as u64;

    // Serialize and write initial header
    let mut buf = Vec::new();
    header
      .write_to(&mut buf)
      .map_err(|e| format!("Failed to serialize header: {}", e))?;
    file
      .write_at(0, &buf)
      .await
      .map_err(|e| format!("Failed to write header: {}", e))?;
    file
      .sync()
      .await
      .map_err(|e| format!("Failed to sync file: {}", e))?;

    Ok(Self {
      name: name.to_string(),
      file: Arc::from(file),
      header,
      index: ChunkIndex::new(),
      body_index: PixelBodyIndex::new(),
      data_write_pos,
      dirty: false,
    })
  }

  /// Opens an existing save file asynchronously (WASM).
  #[cfg(target_family = "wasm")]
  async fn open_async(fs: &dyn StorageFs, name: &str) -> Result<Self, String> {
    let file = fs
      .open(name)
      .await
      .map_err(|e| format!("Failed to open file: {}", e))?;

    // Read header
    let mut header_buf = [0u8; Header::SIZE];
    file
      .read_at(0, &mut header_buf)
      .await
      .map_err(|e| format!("Failed to read header: {}", e))?;
    let header = Header::read_from(&mut Cursor::new(&header_buf))
      .map_err(|e| format!("Invalid header: {}", e))?;
    header
      .validate()
      .map_err(|e| format!("Invalid header: {}", e))?;

    // Read page table
    let page_table_size = header.chunk_count as usize * PageTableEntry::SIZE;
    let mut page_table_buf = vec![0u8; page_table_size];
    file
      .read_at(header.data_region_ptr, &mut page_table_buf)
      .await
      .map_err(|e| format!("Failed to read page table: {}", e))?;
    let index = ChunkIndex::read_from(
      &mut Cursor::new(&page_table_buf),
      header.chunk_count as usize,
    )
    .map_err(|e| format!("Invalid page table: {}", e))?;

    // Read entity section if present
    let body_index = if header.entity_section_ptr != 0 {
      let mut entity_header_buf = [0u8; EntitySectionHeader::SIZE];
      file
        .read_at(header.entity_section_ptr, &mut entity_header_buf)
        .await
        .map_err(|e| format!("Failed to read entity header: {}", e))?;
      let entity_header = EntitySectionHeader::read_from(&mut Cursor::new(&entity_header_buf))
        .map_err(|e| format!("Invalid entity header: {}", e))?;

      let body_index_size = entity_header.entity_count as usize * PixelBodyIndexEntry::SIZE;
      let mut body_index_buf = vec![0u8; body_index_size];
      let body_data_offset = header.entity_section_ptr + EntitySectionHeader::SIZE as u64;
      file
        .read_at(body_data_offset, &mut body_index_buf)
        .await
        .map_err(|e| format!("Failed to read body index: {}", e))?;
      PixelBodyIndex::read_from(
        &mut Cursor::new(&body_index_buf),
        entity_header.entity_count as usize,
      )
      .map_err(|e| format!("Invalid body index: {}", e))?
    } else {
      PixelBodyIndex::new()
    };

    let data_write_pos = header.data_region_ptr;

    Ok(Self {
      name: name.to_string(),
      file: Arc::from(file),
      header,
      index,
      body_index,
      data_write_pos,
      dirty: false,
    })
  }

  /// Returns the save file name.
  pub fn name(&self) -> &str {
    &self.name
  }

  /// Returns the world seed.
  pub fn world_seed(&self) -> u64 {
    self.header.world_seed
  }

  /// Returns true if the given chunk position is persisted.
  pub fn contains(&self, pos: ChunkPos) -> bool {
    self.index.contains(pos)
  }

  /// Returns the number of persisted chunks.
  pub fn chunk_count(&self) -> usize {
    self.index.len()
  }

  /// Returns the number of persisted pixel bodies.
  pub fn body_count(&self) -> usize {
    self.body_index.len()
  }

  /// Returns true if a pixel body with the given ID is persisted.
  pub fn contains_body(&self, stable_id: u64) -> bool {
    self.body_index.contains(stable_id)
  }

  /// Returns all pixel body records for a given chunk.
  pub fn load_bodies_for_chunk(&self, pos: ChunkPos) -> Vec<PixelBodyRecord> {
    let mut records = Vec::new();

    for entry in self.body_index.get_chunk(pos) {
      match self.load_body_record(entry) {
        Ok(record) => records.push(record),
        Err(e) => {
          warn!("Failed to load pixel body {}: {}", entry.stable_id, e);
        }
      }
    }

    records
  }

  /// Loads a single pixel body record by its index entry.
  fn load_body_record(
    &self,
    entry: &PixelBodyIndexEntry,
  ) -> Result<PixelBodyRecord, PixelBodyReadError> {
    let mut buf = vec![0u8; entry.data_size as usize];
    block_on(self.file.read_at(entry.data_offset, &mut buf))
      .map_err(|e| PixelBodyReadError::Io(io::Error::from(e)))?;
    PixelBodyRecord::read_from(&mut Cursor::new(&buf))
  }

  /// Saves a pixel body to the file.
  pub fn save_body(&mut self, record: &PixelBodyRecord) -> io::Result<()> {
    // Serialize to buffer first to get size
    let mut buf = Vec::new();
    record.write_to(&mut buf)?;

    // Write record data at current write position
    block_on(self.file.write_at(self.data_write_pos, &buf)).map_err(io::Error::from)?;

    // Create index entry
    let entry = PixelBodyIndexEntry {
      stable_id: record.stable_id,
      data_offset: self.data_write_pos,
      data_size: buf.len() as u32,
      chunk_pos: record.chunk_pos(),
    };

    // Update state
    self.body_index.insert(entry);
    self.data_write_pos += buf.len() as u64;
    self.dirty = true;

    Ok(())
  }

  /// Removes a pixel body from the index.
  ///
  /// Note: This only removes from the index, not the file data.
  /// Space is reclaimed on next compaction (not yet implemented).
  pub fn remove_body(&mut self, stable_id: u64) {
    if self.body_index.remove(stable_id).is_some() {
      self.dirty = true;
    }
  }

  /// Removes all pixel bodies associated with a chunk.
  pub fn remove_bodies_for_chunk(&mut self, pos: ChunkPos) {
    let removed = self.body_index.remove_chunk(pos);
    if !removed.is_empty() {
      self.dirty = true;
    }
  }

  /// Loads a chunk from the save file.
  ///
  /// Returns None if the chunk is not persisted.
  /// On error, returns None and logs a warning.
  pub fn load_chunk<S: ChunkSeeder>(&self, pos: ChunkPos, _seeder: &S) -> Option<LoadedChunk> {
    let entry = self.index.get(pos)?;

    // Read compressed data
    let mut data = vec![0u8; entry.data_size as usize];
    if let Err(e) = block_on(self.file.read_at(entry.data_offset, &mut data)) {
      warn!("Failed to read chunk {:?}: {}", pos, e);
      return None;
    }

    Some(LoadedChunk {
      storage_type: entry.storage_type,
      data,
      pos,
      seeder_needed: entry.storage_type == StorageType::Delta,
    })
  }

  /// Saves a chunk to the file.
  ///
  /// Computes delta if beneficial, otherwise stores full chunk.
  pub fn save_chunk<S: ChunkSeeder>(
    &mut self,
    chunk: &Chunk,
    pos: ChunkPos,
    seeder: &S,
  ) -> io::Result<()> {
    // Determine storage type
    let deltas = compute_delta(chunk, pos, seeder);
    let (storage_type, data) = if should_use_delta(deltas.len()) {
      (StorageType::Delta, encode_delta(&deltas))
    } else {
      (StorageType::Full, encode_full(chunk))
    };

    // Write size prefix + data
    let size_bytes = (data.len() as u32).to_le_bytes();
    let mut write_buf = Vec::with_capacity(4 + data.len());
    write_buf.extend_from_slice(&size_bytes);
    write_buf.extend_from_slice(&data);
    block_on(self.file.write_at(self.data_write_pos, &write_buf)).map_err(io::Error::from)?;

    // Create page table entry
    let entry = PageTableEntry::new(
      pos,
      self.data_write_pos + 4, // Skip size prefix
      data.len() as u32,
      storage_type,
    );

    // Update state
    self.index.insert(entry);
    self.data_write_pos += 4 + data.len() as u64;
    self.header.chunk_count = self.index.len() as u32;
    self.dirty = true;

    Ok(())
  }

  /// Writes chunk data at the given offset (used by persistence systems).
  #[allow(dead_code)] // Kept for potential future copy-on-write support
  pub(crate) fn write_chunk_data(&self, offset: u64, data: &[u8]) -> io::Result<()> {
    let size_bytes = (data.len() as u32).to_le_bytes();
    let mut write_buf = Vec::with_capacity(4 + data.len());
    write_buf.extend_from_slice(&size_bytes);
    write_buf.extend_from_slice(data);
    block_on(self.file.write_at(offset, &write_buf)).map_err(io::Error::from)
  }

  /// Copies this save to a new name via the storage backend, returning a new
  /// `WorldSave` handle.
  pub fn copy_to(&mut self, fs: &dyn StorageFs, new_name: &str) -> io::Result<WorldSave> {
    // Ensure current file is consistent
    self.flush()?;

    // Copy file
    block_on(fs.copy(&self.name, new_name)).map_err(io::Error::from)?;

    // Open new handle
    WorldSave::open(fs, new_name).map_err(|e| match e {
      OpenError::Io(io_err) => io_err,
      OpenError::Header(h) => io::Error::new(io::ErrorKind::InvalidData, h.to_string()),
    })
  }

  /// Flushes the page table, entity section, and header to disk.
  ///
  /// Rewrites header in-place and appends page table and entity section at end
  /// of file. The page table and entity section locations are stored in the
  /// header.
  pub fn flush(&mut self) -> io::Result<()> {
    if !self.dirty {
      return Ok(());
    }

    // Update header timestamps
    #[cfg(not(target_family = "wasm"))]
    {
      self.header.modified_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    }
    #[cfg(target_family = "wasm")]
    {
      self.header.modified_time = (js_sys::Date::now() / 1000.0) as u64;
    }
    self.header.chunk_count = self.index.len() as u32;
    self.header.page_table_size = self.index.serialized_size() as u32;

    // Page table goes after data region
    self.header.data_region_ptr = self.data_write_pos;

    // Serialize page table
    let mut page_table_buf = Vec::new();
    self.index.write_to(&mut page_table_buf)?;
    block_on(self.file.write_at(self.data_write_pos, &page_table_buf)).map_err(io::Error::from)?;

    // Entity section goes after page table
    let entity_section_start = self.data_write_pos + self.index.serialized_size() as u64;
    self.header.entity_section_ptr = if self.body_index.is_empty() {
      0
    } else {
      entity_section_start
    };

    // Write entity section if we have bodies
    if !self.body_index.is_empty() {
      let entity_header = EntitySectionHeader {
        entity_count: self.body_index.len() as u32,
        _reserved: 0,
      };

      let mut entity_buf = Vec::new();
      entity_header.write_to(&mut entity_buf)?;
      self.body_index.write_to(&mut entity_buf)?;

      block_on(self.file.write_at(entity_section_start, &entity_buf)).map_err(io::Error::from)?;
    }

    // Write updated header
    let mut header_buf = Vec::new();
    self.header.write_to(&mut header_buf)?;
    block_on(self.file.write_at(0, &header_buf)).map_err(io::Error::from)?;

    block_on(self.file.sync()).map_err(io::Error::from)?;
    self.dirty = false;
    Ok(())
  }

  // ===== Async task support methods =====

  /// Returns a clone of the file handle for use in async tasks.
  ///
  /// The file handle is Arc-wrapped and implements positioned I/O,
  /// making it safe to share across tasks.
  pub fn file_handle(&self) -> Arc<dyn StorageFile> {
    Arc::clone(&self.file)
  }

  /// Returns a reference to the chunk index for checking if chunks exist.
  pub fn chunk_index(&self) -> &ChunkIndex {
    &self.index
  }

  /// Returns a reference to the body index.
  pub fn body_index(&self) -> &PixelBodyIndex {
    &self.body_index
  }

  /// Returns the current data write position.
  pub fn data_write_pos(&self) -> u64 {
    self.data_write_pos
  }

  /// Creates a snapshot of indices for passing to async save tasks.
  ///
  /// The snapshot includes clones of the chunk and body indices
  /// that can be safely sent to another thread.
  pub fn create_save_snapshot(&self) -> (ChunkIndex, PixelBodyIndex, u64) {
    (
      self.index.clone(),
      self.body_index.clone(),
      self.data_write_pos,
    )
  }

  /// Merges the result of an async save task back into this WorldSave.
  ///
  /// This replaces the current indices with the updated versions from
  /// the task and updates the write position.
  pub fn merge_save_result(&mut self, result: tasks::SaveResult) {
    self.index = result.chunk_index;
    self.body_index = result.body_index;
    self.data_write_pos = result.data_write_pos;
    self.header.chunk_count = self.index.len() as u32;
    self.dirty = true;
  }
}

/// Loaded chunk data before decompression.
#[derive(Debug)]
pub struct LoadedChunk {
  /// Storage type.
  pub storage_type: StorageType,
  /// Compressed data.
  pub data: Vec<u8>,
  /// Chunk position.
  pub pos: ChunkPos,
  /// Whether the seeder is needed to apply delta.
  pub seeder_needed: bool,
}

impl LoadedChunk {
  /// Applies the loaded data to a chunk.
  ///
  /// For delta storage, the chunk should be pre-seeded.
  pub fn apply_to(&self, chunk: &mut Chunk) -> Result<(), LoadError> {
    match self.storage_type {
      StorageType::Empty => {
        chunk.pixels.fill(crate::pixel::Pixel::VOID);
      }
      StorageType::Delta => {
        let deltas = decode_delta(&self.data).map_err(LoadError::DeltaDecode)?;
        apply_delta(chunk, &deltas);
      }
      StorageType::Full => {
        decode_full(&self.data, chunk).map_err(LoadError::FullDecode)?;
      }
    }
    Ok(())
  }
}

/// Error opening a save file.
#[derive(Debug)]
pub enum OpenError {
  Io(io::Error),
  Header(HeaderError),
}

impl From<io::Error> for OpenError {
  fn from(err: io::Error) -> Self {
    Self::Io(err)
  }
}

impl From<HeaderError> for OpenError {
  fn from(err: HeaderError) -> Self {
    Self::Header(err)
  }
}

impl std::fmt::Display for OpenError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Io(e) => write!(f, "I/O error: {}", e),
      Self::Header(e) => write!(f, "header error: {}", e),
    }
  }
}

impl std::error::Error for OpenError {}

/// Error loading a chunk.
#[derive(Debug)]
pub enum LoadError {
  DeltaDecode(compression::DeltaError),
  FullDecode(compression::FullDecodeError),
}

impl std::fmt::Display for LoadError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::DeltaDecode(e) => write!(f, "delta decode error: {}", e),
      Self::FullDecode(e) => write!(f, "full decode error: {}", e),
    }
  }
}

impl std::error::Error for LoadError {}

/// WASM: Resource for deferred async persistence initialization.
///
/// OPFS requires async setup (awaiting JS promises) which can't be done
/// in synchronous `Plugin::build()`. This resource holds the configuration
/// and task handle while initialization completes in the background.
///
/// The `initialize_wasm_persistence` system polls this task each frame.
/// Once complete, it inserts `PersistenceControl` with the loaded save.
#[cfg(target_family = "wasm")]
#[derive(Resource)]
pub struct PendingWasmPersistence {
  /// World seed for procedural generation fallback.
  pub world_seed: u64,
  /// Save name without extension (e.g., "world").
  pub save_name: String,
  /// Async initialization task handle.
  pub task: Option<bevy::tasks::Task<Result<WasmPersistenceInitResult, String>>>,
}

/// Result of WASM persistence initialization.
#[cfg(target_family = "wasm")]
#[allow(private_interfaces)] // Legacy code, kept for potential future use
pub struct WasmPersistenceInitResult {
  /// The persistence backend.
  pub backend: Arc<dyn backend::PersistenceBackend>,
  /// The opened/created world save.
  pub save: WorldSave,
  /// Save name without extension.
  pub save_name: String,
}

/// Async save task for background saving.
pub struct SaveTask {
  /// Chunk position to save.
  pub pos: ChunkPos,
  /// Compressed data to write.
  pub data: Vec<u8>,
  /// Storage type.
  pub storage_type: StorageType,
}

/// Task for saving a pixel body.
pub struct BodySaveTask {
  /// The pixel body record to save.
  pub record: PixelBodyRecord,
}

/// Task for removing a pixel body from persistence.
pub struct BodyRemoveTask {
  /// Stable ID of the body to remove.
  pub stable_id: u64,
}

/// Resource for pending persistence operations.
#[derive(Resource, Default)]
pub struct PersistenceTasks {
  /// Chunks queued for saving.
  pub save_queue: Vec<SaveTask>,
  /// Pixel bodies queued for saving.
  pub body_save_queue: Vec<BodySaveTask>,
  /// Pixel bodies queued for removal.
  pub body_remove_queue: Vec<BodyRemoveTask>,
}

impl PersistenceTasks {
  /// Queues a chunk for saving.
  pub fn queue_save(&mut self, pos: ChunkPos, data: Vec<u8>, storage_type: StorageType) {
    self.save_queue.push(SaveTask {
      pos,
      data,
      storage_type,
    });
  }

  /// Queues a pixel body for saving.
  pub fn queue_body_save(&mut self, record: PixelBodyRecord) {
    self.body_save_queue.push(BodySaveTask { record });
  }

  /// Queues a pixel body for removal.
  pub fn queue_body_remove(&mut self, stable_id: u64) {
    self.body_remove_queue.push(BodyRemoveTask { stable_id });
  }
}
