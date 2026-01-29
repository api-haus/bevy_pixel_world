//! World control APIs - pause/resume simulation and on-demand persistence.
//!
//! Provides public APIs for:
//! - Pausing/resuming world simulation and physics
//! - Triggering on-demand persistence with completion notification

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use bevy::prelude::*;

use crate::persistence::WorldSave;
use crate::persistence::backend::PersistenceBackend;

/// Controls whether world simulation is running or paused.
///
/// When paused:
/// - Cellular automata simulation stops
/// - Pixel body physics is disabled (if using avian2d/rapier2d)
/// - Rendering continues (world remains visible)
/// - Persistence operations can still run
///
/// # Example
/// ```ignore
/// fn pause_menu_system(
///     keys: Res<ButtonInput<KeyCode>>,
///     mut sim_state: ResMut<SimulationState>,
/// ) {
///     if keys.just_pressed(KeyCode::Escape) {
///         sim_state.toggle();
///     }
/// }
/// ```
#[derive(Resource, Debug, Default)]
pub struct SimulationState {
  paused: bool,
}

impl SimulationState {
  /// Creates a new simulation state (running by default).
  pub fn new() -> Self {
    Self::default()
  }

  /// Creates a paused simulation state.
  pub fn paused() -> Self {
    Self { paused: true }
  }

  /// Returns true if simulation is paused.
  pub fn is_paused(&self) -> bool {
    self.paused
  }

  /// Returns true if simulation is running.
  pub fn is_running(&self) -> bool {
    !self.paused
  }

  /// Pauses the simulation.
  pub fn pause(&mut self) {
    self.paused = true;
  }

  /// Resumes the simulation.
  pub fn resume(&mut self) {
    self.paused = false;
  }

  /// Toggles between paused and running.
  pub fn toggle(&mut self) {
    self.paused = !self.paused;
  }

  /// Sets the paused state.
  pub fn set_paused(&mut self, paused: bool) {
    self.paused = paused;
  }
}

/// Resource for persistence control.
///
/// Provides methods to save the world to the current file or copy to a new
/// path.
#[derive(Resource)]
pub struct PersistenceControl {
  /// Persistence backend (native only; None on WASM where IoDispatcher handles
  /// I/O).
  pub(crate) backend: Option<Arc<dyn PersistenceBackend>>,
  /// The loaded WorldSave (native only).
  pub(crate) world_save: Option<Arc<RwLock<WorldSave>>>,
  /// Current save file path.
  pub(crate) current_path: Option<PathBuf>,
  /// Counter for generating unique request IDs.
  next_request_id: u64,
  /// Pending persistence requests.
  pub(crate) pending_requests: Vec<PersistenceRequestInner>,
}

impl PersistenceControl {
  /// Creates a persistence control with an already-loaded save.
  ///
  /// Used during synchronous native initialization where save is opened
  /// immediately.
  pub(crate) fn with_save(
    backend: Arc<dyn PersistenceBackend>,
    world_save: WorldSave,
    path: PathBuf,
  ) -> Self {
    Self {
      backend: Some(backend),
      world_save: Some(Arc::new(RwLock::new(world_save))),
      current_path: Some(path),
      next_request_id: 1,
      pending_requests: Vec::new(),
    }
  }

  /// Creates a persistence control that only tracks the path.
  ///
  /// Used on WASM where I/O is handled by IoDispatcher instead of direct file
  /// access.
  pub fn with_path_only(path: PathBuf) -> Self {
    Self {
      backend: None,
      world_save: None,
      current_path: Some(path),
      next_request_id: 1,
      pending_requests: Vec::new(),
    }
  }

  /// Returns true if a save file is open and ready for I/O.
  ///
  /// Check this before calling save methods.
  pub fn is_active(&self) -> bool {
    self.current_path.is_some()
  }

  /// Returns a reference to the loaded save, if any.
  pub(crate) fn world_save(&self) -> Option<&Arc<RwLock<WorldSave>>> {
    self.world_save.as_ref()
  }

  /// Saves all chunks and pixel bodies to the current save file.
  ///
  /// Returns a handle that can be polled to check completion.
  ///
  /// # Panics
  /// Panics if persistence is not active. Check `is_active()` first.
  ///
  /// # Example
  /// ```ignore
  /// fn save_system(mut ctrl: ResMut<PersistenceControl>) {
  ///     if !ctrl.is_active() {
  ///         return; // No save file open
  ///     }
  ///     let handle = ctrl.save();
  /// }
  /// ```
  pub fn save(&mut self) -> PersistenceHandle {
    assert!(
      self.is_active(),
      "save() called when persistence is not active"
    );
    self.save_internal(None)
  }

  /// Saves to a new path (copy-on-write).
  ///
  /// The current save is copied to the new path, then the new path becomes
  /// the active save file. Use this for "Save As" functionality.
  ///
  /// Returns a handle that can be polled to check completion.
  ///
  /// # Panics
  /// Panics if persistence is not active. Check `is_active()` first.
  ///
  /// # Example
  /// ```ignore
  /// fn save_as_system(mut ctrl: ResMut<PersistenceControl>) {
  ///     if !ctrl.is_active() {
  ///         return;
  ///     }
  ///     // Copy current save to backup, then continue saving to backup
  ///     ctrl.save_to("/home/user/saves/backup.save");
  /// }
  /// ```
  pub fn save_to(&mut self, path: impl Into<PathBuf>) -> PersistenceHandle {
    assert!(
      self.is_active(),
      "save_to() called when persistence is not active"
    );
    self.save_internal(Some(path.into()))
  }

  /// Internal helper for save operations.
  fn save_internal(&mut self, target_path: Option<PathBuf>) -> PersistenceHandle {
    let id = self.next_request_id;
    self.next_request_id += 1;

    let completed = Arc::new(AtomicBool::new(false));
    let request = PersistenceRequestInner {
      id,
      completed: completed.clone(),
      target_path,
    };
    self.pending_requests.push(request);

    PersistenceHandle { id, completed }
  }

  /// Copies the current save to a new path (internal copy-on-write
  /// implementation).
  ///
  /// Updates the current path to point to the copied file.
  pub(crate) fn copy_to(&mut self, target_path: &Path) -> std::io::Result<()> {
    let backend = self.backend.as_ref().ok_or_else(|| {
      std::io::Error::new(std::io::ErrorKind::Unsupported, "no backend available")
    })?;

    let save_arc = self
      .world_save
      .as_ref()
      .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no save loaded"))?;

    let mut save = save_arc
      .write()
      .map_err(|_| std::io::Error::other("save lock poisoned"))?;

    // Extract filename from target path for backend
    let file_name = target_path
      .file_name()
      .and_then(|f| f.to_str())
      .ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid target path")
      })?;

    let new_save = backend.save_copy(&mut save, file_name)?;

    // Replace the current save with the new one
    drop(save);
    self.world_save = Some(Arc::new(RwLock::new(new_save)));
    self.current_path = Some(target_path.to_path_buf());

    Ok(())
  }
}

/// Internal representation of a pending persistence request.
pub(crate) struct PersistenceRequestInner {
  pub id: u64,
  pub completed: Arc<AtomicBool>,
  /// Target path for copy-on-write. None = save to current path.
  pub target_path: Option<PathBuf>,
}

/// Handle to track completion of an on-demand save operation.
///
/// Can be polled synchronously or used with async patterns.
#[derive(Clone)]
pub struct PersistenceHandle {
  id: u64,
  completed: Arc<AtomicBool>,
}

impl PersistenceHandle {
  /// Returns the unique ID of this save request.
  pub fn id(&self) -> u64 {
    self.id
  }

  /// Returns true if the save operation has completed.
  pub fn is_complete(&self) -> bool {
    self.completed.load(Ordering::Acquire)
  }

  /// Blocks until the save operation completes.
  ///
  /// **Warning**: This will block the current thread. In a Bevy system,
  /// prefer polling `is_complete()` across frames instead.
  pub fn wait(&self) {
    while !self.is_complete() {
      std::thread::yield_now();
    }
  }

  /// Returns a future that resolves when the save completes.
  ///
  /// Can be used with `bevy::tasks::block_on` or async executors.
  pub fn into_future(self) -> PersistenceFuture {
    PersistenceFuture {
      completed: self.completed,
    }
  }
}

/// Future that resolves when a persistence operation completes.
pub struct PersistenceFuture {
  completed: Arc<AtomicBool>,
}

impl std::future::Future for PersistenceFuture {
  type Output = ();

  fn poll(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    if self.completed.load(Ordering::Acquire) {
      std::task::Poll::Ready(())
    } else {
      // Wake immediately to poll again (busy-wait, but works across threads)
      cx.waker().wake_by_ref();
      std::task::Poll::Pending
    }
  }
}

/// Message emitted when a persistence operation completes.
#[derive(bevy::prelude::Message, Clone, Debug)]
pub struct PersistenceComplete {
  /// The request ID that completed.
  pub request_id: u64,
  /// Whether the operation succeeded.
  pub success: bool,
  /// Error message if failed.
  pub error: Option<String>,
}

/// Message to request an immediate save.
///
/// Alternative to using `PersistenceControl::save()`.
/// Useful when you need to trigger a save from a system that doesn't
/// have mutable access to `PersistenceControl`.
#[derive(bevy::prelude::Message, Clone, Debug, Default)]
pub struct RequestPersistence;

/// Marker resource indicating the I/O dispatcher is ready (WASM only).
///
/// On WASM, persistence uses a Web Worker instead of PersistenceControl's
/// internal WorldSave. This marker indicates the worker has initialized.
#[derive(Resource)]
pub struct IoDispatcherReady;

/// Resource holding pending persistence initialization data (WASM only).
///
/// When the I/O worker sends back `Initialized`, this is consumed to create
/// the `PersistenceControl` resource.
#[derive(Resource)]
pub struct PendingPersistenceInit {
  /// Save file path.
  pub path: PathBuf,
  /// World seed.
  pub world_seed: u64,
}
