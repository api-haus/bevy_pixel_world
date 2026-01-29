//! Native I/O worker using a dedicated thread and async-channel.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, JoinHandle};

use async_channel::{Receiver, Sender, TryRecvError};

use super::{ChunkLoadData, IoCommand, IoResult};
use crate::persistence::format::{PageTableEntry, StorageType};
use crate::persistence::index::{ChunkIndex, PixelBodyIndex, PixelBodyIndexEntry};
use crate::persistence::native::NativeFs;
use crate::persistence::{PixelBodyRecord, WorldSave};

/// Native I/O dispatcher using a background thread.
pub struct NativeIoDispatcher {
  cmd_tx: Sender<IoCommand>,
  result_rx: Receiver<IoResult>,
  ready: Arc<AtomicBool>,
  world_seed: Arc<AtomicU64>,
  _worker_handle: JoinHandle<()>,
}

impl NativeIoDispatcher {
  /// Creates a new native I/O dispatcher with a worker thread.
  pub fn new(save_dir: PathBuf) -> Self {
    let (cmd_tx, cmd_rx) = async_channel::unbounded::<IoCommand>();
    let (result_tx, result_rx) = async_channel::unbounded::<IoResult>();
    let ready = Arc::new(AtomicBool::new(false));
    let world_seed = Arc::new(AtomicU64::new(0));

    let worker_handle = thread::spawn(move || {
      worker_loop(save_dir, cmd_rx, result_tx);
    });

    Self {
      cmd_tx,
      result_rx,
      ready,
      world_seed,
      _worker_handle: worker_handle,
    }
  }

  /// Sends a command to the worker.
  pub fn send(&self, cmd: IoCommand) {
    // Use try_send since we're in sync context
    let _ = self.cmd_tx.send_blocking(cmd);
  }

  /// Tries to receive a result from the worker.
  pub fn try_recv(&self) -> Option<IoResult> {
    match self.result_rx.try_recv() {
      Ok(result) => Some(result),
      Err(TryRecvError::Empty) => None,
      Err(TryRecvError::Closed) => None,
    }
  }

  /// Returns true if the worker is initialized.
  pub fn is_ready(&self) -> bool {
    self.ready.load(Ordering::Acquire)
  }

  /// Sets the ready state.
  pub fn set_ready(&self, ready: bool) {
    self.ready.store(ready, Ordering::Release);
  }

  /// Returns the world seed if set.
  pub fn world_seed(&self) -> Option<u64> {
    let seed = self.world_seed.load(Ordering::Acquire);
    if seed == 0 && !self.is_ready() {
      None
    } else {
      Some(seed)
    }
  }

  /// Sets the world seed.
  pub fn set_world_seed(&self, seed: u64) {
    self.world_seed.store(seed, Ordering::Release);
  }
}

/// Worker state maintained across commands.
struct WorkerState {
  fs: NativeFs,
  save: Option<WorldSave>,
  chunk_index: ChunkIndex,
  body_index: PixelBodyIndex,
  data_write_pos: u64,
}

impl WorkerState {
  fn new(save_dir: PathBuf) -> std::io::Result<Self> {
    let fs = NativeFs::new(save_dir)?;
    Ok(Self {
      fs,
      save: None,
      chunk_index: ChunkIndex::new(),
      body_index: PixelBodyIndex::new(),
      data_write_pos: 0,
    })
  }
}

/// Main worker loop running in dedicated thread.
fn worker_loop(save_dir: PathBuf, cmd_rx: Receiver<IoCommand>, result_tx: Sender<IoResult>) {
  let mut state = match WorkerState::new(save_dir) {
    Ok(s) => s,
    Err(e) => {
      let _ = result_tx.send_blocking(IoResult::Error {
        message: format!("Failed to initialize worker: {}", e),
      });
      return;
    }
  };

  while let Ok(cmd) = cmd_rx.recv_blocking() {
    let result = handle_command(&mut state, cmd);

    // Check if we should shutdown
    let should_shutdown = matches!(result, IoResult::Error { .. }) && result_tx.is_closed();

    let _ = result_tx.send_blocking(result);

    if should_shutdown {
      break;
    }
  }
}

/// Handles a single command and returns the result.
fn handle_command(state: &mut WorkerState, cmd: IoCommand) -> IoResult {
  match cmd {
    IoCommand::Initialize { save_name, seed } => handle_initialize(state, save_name, seed),
    IoCommand::LoadChunk { chunk_pos } => handle_load_chunk(state, chunk_pos),
    IoCommand::WriteChunk { chunk_pos, data } => handle_write_chunk(state, chunk_pos, data),
    IoCommand::SaveBody {
      record_data,
      stable_id,
    } => handle_save_body(state, record_data, stable_id),
    IoCommand::RemoveBody { stable_id } => handle_remove_body(state, stable_id),
    IoCommand::Flush => handle_flush(state),
    IoCommand::Shutdown => {
      // Flush before shutdown
      let _ = handle_flush(state);
      IoResult::FlushComplete
    }
  }
}

fn handle_initialize(state: &mut WorkerState, save_name: String, seed: u64) -> IoResult {
  let file_name = format!("{}.save", save_name);

  match WorldSave::open_or_create(&state.fs, &file_name, seed) {
    Ok(save) => {
      let chunk_count = save.chunk_count();
      let body_count = save.body_count();
      let world_seed = save.world_seed();

      // Copy indices to worker state
      state.chunk_index = save.chunk_index().clone();
      state.body_index = save.body_index().clone();
      state.data_write_pos = save.data_write_pos();
      state.save = Some(save);

      IoResult::Initialized {
        chunk_count,
        body_count,
        world_seed,
      }
    }
    Err(e) => IoResult::Error {
      message: format!("Failed to open/create save '{}': {}", save_name, e),
    },
  }
}

fn handle_load_chunk(state: &mut WorkerState, chunk_pos: bevy::math::IVec2) -> IoResult {
  let pos = crate::coords::ChunkPos::new(chunk_pos.x, chunk_pos.y);

  let Some(entry) = state.chunk_index.get(pos) else {
    return IoResult::ChunkLoaded {
      chunk_pos,
      data: None,
    };
  };

  let Some(ref save) = state.save else {
    return IoResult::Error {
      message: "No save loaded".to_string(),
    };
  };

  // Read compressed data
  let mut data = vec![0u8; entry.data_size as usize];
  if let Err(e) = crate::persistence::block_on(save.file.read_at(entry.data_offset, &mut data)) {
    return IoResult::Error {
      message: format!("Failed to read chunk {:?}: {}", pos, e),
    };
  }

  IoResult::ChunkLoaded {
    chunk_pos,
    data: Some(ChunkLoadData {
      storage_type: entry.storage_type as u8,
      data,
      seeder_needed: entry.storage_type == StorageType::Delta,
    }),
  }
}

fn handle_write_chunk(
  state: &mut WorkerState,
  chunk_pos: bevy::math::IVec2,
  data: Vec<u8>,
) -> IoResult {
  let pos = crate::coords::ChunkPos::new(chunk_pos.x, chunk_pos.y);

  let Some(ref save) = state.save else {
    return IoResult::Error {
      message: "No save loaded".to_string(),
    };
  };

  // Write size prefix + data
  let size_bytes = (data.len() as u32).to_le_bytes();
  let mut write_buf = Vec::with_capacity(4 + data.len());
  write_buf.extend_from_slice(&size_bytes);
  write_buf.extend_from_slice(&data);

  if let Err(e) = crate::persistence::block_on(save.file.write_at(state.data_write_pos, &write_buf))
  {
    return IoResult::Error {
      message: format!("Failed to write chunk {:?}: {}", pos, e),
    };
  }

  // Create page table entry
  let entry = PageTableEntry::new(
    pos,
    state.data_write_pos + 4, // Skip size prefix
    data.len() as u32,
    StorageType::Full,
  );

  // Update state
  state.chunk_index.insert(entry);
  state.data_write_pos += 4 + data.len() as u64;

  IoResult::WriteComplete { chunk_pos }
}

fn handle_save_body(state: &mut WorkerState, record_data: Vec<u8>, stable_id: u64) -> IoResult {
  let Some(ref save) = state.save else {
    return IoResult::Error {
      message: "No save loaded".to_string(),
    };
  };

  // Parse the record to get chunk_pos
  let record = match PixelBodyRecord::read_from(&mut std::io::Cursor::new(&record_data)) {
    Ok(r) => r,
    Err(e) => {
      return IoResult::Error {
        message: format!("Failed to parse body record {}: {}", stable_id, e),
      };
    }
  };

  // Write to file
  if let Err(e) =
    crate::persistence::block_on(save.file.write_at(state.data_write_pos, &record_data))
  {
    return IoResult::Error {
      message: format!("Failed to write body {}: {}", stable_id, e),
    };
  }

  // Create index entry
  let entry = PixelBodyIndexEntry {
    stable_id,
    data_offset: state.data_write_pos,
    data_size: record_data.len() as u32,
    chunk_pos: record.chunk_pos(),
  };

  // Update state
  state.body_index.insert(entry);
  state.data_write_pos += record_data.len() as u64;

  IoResult::BodySaveComplete { stable_id }
}

fn handle_remove_body(state: &mut WorkerState, stable_id: u64) -> IoResult {
  state.body_index.remove(stable_id);
  IoResult::BodyRemoveComplete { stable_id }
}

fn handle_flush(state: &mut WorkerState) -> IoResult {
  let Some(ref mut save) = state.save else {
    return IoResult::Error {
      message: "No save loaded".to_string(),
    };
  };

  // Update save's indices from worker state
  save.index = state.chunk_index.clone();
  save.body_index = state.body_index.clone();
  save.data_write_pos = state.data_write_pos;
  save.dirty = true;

  if let Err(e) = save.flush() {
    return IoResult::Error {
      message: format!("Failed to flush: {}", e),
    };
  }

  IoResult::FlushComplete
}
