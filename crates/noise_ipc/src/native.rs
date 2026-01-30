//! Native (non-WASM) implementation using POSIX shared memory.

use std::ffi::CStr;
use std::sync::atomic::{fence, Ordering};

use shared_memory::{Shmem, ShmemConf, ShmemError};

const SHM_NAME: &str = "/FastNoise2NodeEditor";
const SHM_SIZE: usize = 64 * 1024;

/// IPC data types for communication with NoiseTool.
#[repr(u8)]
pub enum IpcDataType {
  /// Preview only - NoiseTool writes selected node ENT (ignored by editor).
  Preview = 0,
  /// Clear graph and import ENT as editable nodes (editor → NoiseTool).
  Import = 1,
  /// Explicit apply - user clicked "Apply to Editor" (NoiseTool → editor).
  Apply = 2,
}

/// IPC client for communicating with FastNoise2 NoiseTool.
///
/// # Safety
///
/// The underlying shared memory is process-global and the IPC protocol uses
/// an atomic counter for synchronization. It is safe to access from multiple
/// threads, though concurrent writes should be avoided (only one writer at a
/// time).
pub struct NoiseIpc {
  shmem: Shmem,
  last_counter: u8,
}

// Safety: The shared memory segment is process-global memory-mapped region.
// Access is synchronized via the atomic counter in the IPC protocol.
// The Shmem contains raw pointers to mmap'd memory which is inherently
// thread-safe.
unsafe impl Send for NoiseIpc {}
unsafe impl Sync for NoiseIpc {}

impl NoiseIpc {
  /// Create a new IPC client, opening or creating the shared memory segment.
  pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
    // Try to open existing shared memory first (NoiseTool may have created it)
    let shmem = match ShmemConf::new().os_id(SHM_NAME).open() {
      Ok(shmem) => shmem,
      Err(ShmemError::LinkDoesNotExist) => {
        // Create new shared memory segment
        ShmemConf::new().size(SHM_SIZE).os_id(SHM_NAME).create()?
      }
      Err(e) => return Err(e.into()),
    };

    Ok(Self {
      shmem,
      last_counter: 0,
    })
  }

  /// Poll for explicit "Apply to Editor" updates from NoiseTool.
  ///
  /// Returns `Some(ent)` only when user clicked "Apply to Editor" in NoiseTool
  /// (type 2). Preview updates (type 0) are ignored.
  pub fn poll(&mut self) -> Option<String> {
    let ptr = self.shmem.as_ptr();
    unsafe {
      let counter = *ptr;
      if counter != self.last_counter {
        self.last_counter = counter;
        let data_type = *ptr.add(1);
        // Only accept explicit "Apply to Editor" (type 2), not preview updates (type 0)
        if data_type == IpcDataType::Apply as u8 {
          let cstr = CStr::from_ptr(ptr.add(2) as *const i8);
          return Some(cstr.to_string_lossy().into_owned());
        }
      }
    }
    None
  }

  /// Send "clear + import" command to NoiseTool (type 1).
  ///
  /// This clears NoiseTool's graph and imports the given ENT as editable nodes.
  pub fn send_import(&mut self, ent: &str) {
    let ptr = self.shmem.as_ptr() as *mut u8;
    let ent_bytes = ent.as_bytes();

    // Ensure ENT fits in shared memory (leaving room for counter, type, and null
    // terminator)
    let max_len = SHM_SIZE - 3;
    let len = ent_bytes.len().min(max_len);

    unsafe {
      // Write ENT string at offset 2
      std::ptr::copy_nonoverlapping(ent_bytes.as_ptr(), ptr.add(2), len);
      // Null terminator
      *ptr.add(2 + len) = 0;

      // Set type = 1 (import)
      *ptr.add(1) = IpcDataType::Import as u8;

      // Memory fence to ensure writes are visible before counter update
      fence(Ordering::Release);

      // Increment counter to signal update
      *ptr = (*ptr).wrapping_add(1);
    }
  }
}
