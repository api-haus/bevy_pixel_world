//! Chunk persistence - save/load modified chunks to disk.
//!
//! This module provides disk storage for modified chunks, allowing player
//! modifications to persist across sessions while procedurally regenerating
//! unmodified areas.
//!
//! See `docs/architecture/chunk-persistence.md` for the full specification.

pub mod compression;
pub mod format;
pub mod index;

use std::fs::{self, File};
use std::io::{self, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bevy::prelude::*;
use compression::{
  apply_delta, compute_delta, decode_delta, decode_full, encode_delta, encode_full,
  should_use_delta,
};
use format::{Header, HeaderError, PageTableEntry, StorageType};
use index::ChunkIndex;

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
pub fn default_save_dir(app_name: &str) -> PathBuf {
  dirs::data_dir()
    .unwrap_or_else(|| PathBuf::from("."))
    .join(app_name)
    .join("saves")
}

/// World save file handle with runtime index.
///
/// Holds the open save file and in-memory index for O(1) chunk lookups.
pub struct WorldSave {
  /// Path to the save file.
  pub(crate) path: PathBuf,
  /// File header.
  pub(crate) header: Header,
  /// Runtime index mapping positions to page table entries.
  pub(crate) index: ChunkIndex,
  /// Current write position in data region (for append).
  pub(crate) data_write_pos: u64,
  /// Whether the save has been modified since last flush.
  pub(crate) dirty: bool,
}

impl WorldSave {
  /// Creates a new save file at the given path.
  ///
  /// Creates the parent directory if it doesn't exist.
  pub fn create(path: impl AsRef<Path>, world_seed: u64) -> io::Result<Self> {
    let path = path.as_ref().to_path_buf();

    // Create parent directory
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent)?;
    }

    let header = Header::new(world_seed);
    // Data starts right after header (64 bytes)
    let data_write_pos = Header::SIZE as u64;

    // Write initial header
    let mut file = File::create(&path)?;
    header.write_to(&mut file)?;
    file.sync_all()?;

    Ok(Self {
      path,
      header,
      index: ChunkIndex::new(),
      data_write_pos,
      dirty: false,
    })
  }

  /// Opens an existing save file.
  pub fn open(path: impl AsRef<Path>) -> Result<Self, OpenError> {
    let path = path.as_ref().to_path_buf();
    let mut file = BufReader::new(File::open(&path)?);

    // Read and validate header
    let header = Header::read_from(&mut file)?;
    header.validate()?;

    // Page table is at data_region_ptr (end of data)
    file.seek(SeekFrom::Start(header.data_region_ptr))?;

    // Read page table
    let index = ChunkIndex::read_from(&mut file, header.chunk_count as usize)?;

    // Data write position is where the page table currently is
    // (page table will be rewritten on flush)
    let data_write_pos = header.data_region_ptr;

    Ok(Self {
      path,
      header,
      index,
      data_write_pos,
      dirty: false,
    })
  }

  /// Opens an existing save file or creates a new one.
  pub fn open_or_create(path: impl AsRef<Path>, world_seed: u64) -> Result<Self, OpenError> {
    let path = path.as_ref();
    if path.exists() {
      Self::open(path)
    } else {
      Ok(Self::create(path, world_seed)?)
    }
  }

  /// Returns the save file path.
  pub fn path(&self) -> &Path {
    &self.path
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

  /// Loads a chunk from the save file.
  ///
  /// Returns None if the chunk is not persisted.
  /// On error, returns None and logs a warning.
  pub fn load_chunk<S: ChunkSeeder>(&self, pos: ChunkPos, _seeder: &S) -> Option<LoadedChunk> {
    let entry = self.index.get(pos)?;

    // Open file and seek to data
    let mut file = match File::open(&self.path) {
      Ok(f) => BufReader::new(f),
      Err(e) => {
        eprintln!(
          "Warning: failed to open save file for chunk {:?}: {}",
          pos, e
        );
        return None;
      }
    };

    if let Err(e) = file.seek(SeekFrom::Start(entry.data_offset)) {
      eprintln!("Warning: failed to seek to chunk {:?}: {}", pos, e);
      return None;
    }

    // Read compressed data
    let mut data = vec![0u8; entry.data_size as usize];
    if let Err(e) = file.read_exact(&mut data) {
      eprintln!("Warning: failed to read chunk {:?}: {}", pos, e);
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

    // Open file for append
    let mut file = File::options().write(true).open(&self.path)?;

    // Seek to write position
    file.seek(SeekFrom::Start(self.data_write_pos))?;

    // Write entry size prefix (for forward iteration during recovery)
    let size_bytes = (data.len() as u32).to_le_bytes();
    file.write_all(&size_bytes)?;
    file.write_all(&data)?;

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

  /// Flushes the page table and header to disk.
  ///
  /// Rewrites header in-place and appends page table at end of file.
  /// The page table location is stored in the header.
  pub fn flush(&mut self) -> io::Result<()> {
    if !self.dirty {
      return Ok(());
    }

    let mut file = File::options().read(true).write(true).open(&self.path)?;

    // Update header timestamps
    self.header.modified_time = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .map(|d| d.as_secs())
      .unwrap_or(0);
    self.header.chunk_count = self.index.len() as u32;
    self.header.page_table_size = self.index.serialized_size() as u32;

    // Page table goes after data region
    self.header.data_region_ptr = self.data_write_pos;

    // Write updated header
    file.seek(SeekFrom::Start(0))?;
    self.header.write_to(&mut file)?;

    // Write page table at end of data region
    file.seek(SeekFrom::Start(self.data_write_pos))?;
    self.index.write_to(&mut file)?;

    file.sync_all()?;
    self.dirty = false;
    Ok(())
  }
}

/// Loaded chunk data before decompression.
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
        // Fill with void
        for y in 0..chunk.pixels.height() {
          for x in 0..chunk.pixels.width() {
            chunk.pixels[(x, y)] = crate::pixel::Pixel::VOID;
          }
        }
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

/// Bevy resource holding the world save.
#[derive(Resource)]
pub struct WorldSaveResource {
  /// The world save file handle.
  pub save: Arc<std::sync::RwLock<WorldSave>>,
}

impl WorldSaveResource {
  /// Creates a new resource wrapping the save.
  pub fn new(save: WorldSave) -> Self {
    Self {
      save: Arc::new(std::sync::RwLock::new(save)),
    }
  }
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

/// Resource for pending persistence operations.
#[derive(Resource, Default)]
pub struct PersistenceTasks {
  /// Chunks queued for saving.
  pub save_queue: Vec<SaveTask>,
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
}
