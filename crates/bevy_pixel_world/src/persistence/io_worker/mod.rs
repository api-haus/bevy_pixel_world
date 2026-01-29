//! Background I/O worker for persistence operations.
//!
//! Provides platform-specific I/O workers:
//! - **Native**: Worker thread with `async-channel`
//! - **WASM**: Web Worker with `postMessage`
//!
//! The main thread communicates via [`IoDispatcher`], sending commands
//! and receiving results asynchronously.

#[cfg(not(target_family = "wasm"))]
mod native;
#[cfg(target_family = "wasm")]
mod wasm;

use std::path::PathBuf;

use bevy::math::IVec2;
use bevy::prelude::*;
#[cfg(not(target_family = "wasm"))]
pub use native::NativeIoDispatcher;
#[cfg(target_family = "wasm")]
pub use wasm::WasmIoDispatcher;

/// Commands sent from main thread to I/O worker.
#[derive(Debug, Clone)]
pub enum IoCommand {
  /// Initialize persistence with save file path and seed.
  /// On WASM, only the filename portion is used (OPFS is a flat store).
  Initialize { path: PathBuf, seed: u64 },
  /// Load chunk data and associated bodies from storage.
  LoadChunk { chunk_pos: IVec2 },
  /// Write chunk data to storage.
  WriteChunk { chunk_pos: IVec2, data: Vec<u8> },
  /// Save a pixel body.
  SaveBody {
    record_data: Vec<u8>,
    stable_id: u64,
  },
  /// Remove a pixel body from persistence.
  RemoveBody { stable_id: u64 },
  /// Flush all pending writes to disk.
  Flush,
  /// Shutdown the worker.
  Shutdown,
}

/// Results received from I/O worker.
#[derive(Debug, Clone)]
pub enum IoResult {
  /// Initialization complete.
  Initialized {
    chunk_count: usize,
    body_count: usize,
    world_seed: u64,
  },
  /// Chunk data and bodies loaded.
  ChunkLoaded {
    chunk_pos: IVec2,
    /// Chunk pixel data (None if not found).
    data: Option<ChunkLoadData>,
    /// Bodies associated with this chunk.
    bodies: Vec<BodyLoadData>,
  },
  /// Write completed.
  WriteComplete { chunk_pos: IVec2 },
  /// Body save completed.
  BodySaveComplete { stable_id: u64 },
  /// Body removal completed.
  BodyRemoveComplete { stable_id: u64 },
  /// Flush completed.
  FlushComplete,
  /// Error occurred.
  Error { message: String },
}

/// Loaded body data from worker.
#[derive(Debug, Clone)]
pub struct BodyLoadData {
  /// Serialized PixelBodyRecord.
  pub record_data: Vec<u8>,
}

/// Loaded chunk data from worker.
#[derive(Debug, Clone)]
pub struct ChunkLoadData {
  /// Storage type (Full or Delta).
  pub storage_type: u8,
  /// Compressed chunk data.
  pub data: Vec<u8>,
  /// Whether seeder is needed (for delta encoding).
  pub seeder_needed: bool,
}

/// Main thread interface for I/O worker communication.
///
/// Wraps platform-specific dispatcher implementations.
#[derive(Resource)]
pub struct IoDispatcher {
  #[cfg(not(target_family = "wasm"))]
  inner: NativeIoDispatcher,
  #[cfg(target_family = "wasm")]
  inner: WasmIoDispatcher,
}

impl IoDispatcher {
  /// Creates a new IoDispatcher with the given save directory (native) or OPFS
  /// root (WASM).
  #[cfg(not(target_family = "wasm"))]
  pub fn new(save_dir: std::path::PathBuf) -> Self {
    Self {
      inner: NativeIoDispatcher::new(save_dir),
    }
  }

  /// Creates a new IoDispatcher for WASM (uses OPFS).
  #[cfg(target_family = "wasm")]
  pub fn new() -> Self {
    Self {
      inner: WasmIoDispatcher::new(),
    }
  }

  /// Sends a command to the I/O worker.
  pub fn send(&self, cmd: IoCommand) {
    self.inner.send(cmd);
  }

  /// Tries to receive a result from the I/O worker.
  /// Returns None if no results are available.
  pub fn try_recv(&self) -> Option<IoResult> {
    self.inner.try_recv()
  }

  /// Returns true if the worker is initialized and ready.
  pub fn is_ready(&self) -> bool {
    self.inner.is_ready()
  }

  /// Sets the ready state (called when Initialized result is received).
  pub fn set_ready(&self, ready: bool) {
    self.inner.set_ready(ready);
  }

  /// Returns the world seed if initialized, None otherwise.
  pub fn world_seed(&self) -> Option<u64> {
    self.inner.world_seed()
  }

  /// Sets the world seed (called when Initialized result is received).
  pub fn set_world_seed(&self, seed: u64) {
    self.inner.set_world_seed(seed);
  }

  /// Returns the initialization counts (chunk_count, body_count).
  /// Returns (0, 0) if not yet initialized.
  pub fn init_counts(&self) -> (usize, usize) {
    self.inner.init_counts()
  }

  /// Sets the initialization counts (called when Initialized result is
  /// received).
  pub fn set_init_counts(&self, chunk_count: usize, body_count: usize) {
    self.inner.set_init_counts(chunk_count, body_count);
  }
}
