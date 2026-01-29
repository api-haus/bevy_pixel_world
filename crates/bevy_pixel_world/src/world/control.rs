//! World control APIs - pause/resume simulation and on-demand persistence.
//!
//! Provides public APIs for:
//! - Pausing/resuming world simulation and physics
//! - Triggering on-demand persistence with completion notification
//! - Managing named saves

use std::io;
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

/// Resource for persistence control and named save management.
#[derive(Resource)]
pub struct PersistenceControl {
  /// Persistence backend.
  backend: Option<Arc<dyn PersistenceBackend>>,
  /// The loaded WorldSave.
  world_save: Option<Arc<RwLock<WorldSave>>>,
  /// Save name without .save extension (e.g., "world").
  pub(crate) save_name: Option<String>,
  /// Counter for generating unique request IDs.
  next_request_id: u64,
  /// Pending persistence requests.
  pub(crate) pending_requests: Vec<PersistenceRequestInner>,
}

impl PersistenceControl {
  /// Creates a new persistence control with the given backend but no loaded
  /// save.
  pub fn new(backend: Arc<dyn PersistenceBackend>) -> Self {
    Self {
      backend: Some(backend),
      world_save: None,
      save_name: None,
      next_request_id: 1,
      pending_requests: Vec::new(),
    }
  }

  /// Creates a persistence control with an already-loaded save.
  ///
  /// Used during synchronous native initialization where save is opened
  /// immediately.
  pub fn with_save(
    backend: Arc<dyn PersistenceBackend>,
    world_save: WorldSave,
    save_name: String,
  ) -> Self {
    Self {
      backend: Some(backend),
      world_save: Some(Arc::new(RwLock::new(world_save))),
      save_name: Some(save_name),
      next_request_id: 1,
      pending_requests: Vec::new(),
    }
  }

  /// Creates a persistence control that only tracks the save name.
  ///
  /// Used when I/O is handled by IoDispatcher instead of direct file access.
  pub fn with_name_only(save_name: String) -> Self {
    Self {
      backend: None,
      world_save: None,
      save_name: Some(save_name),
      next_request_id: 1,
      pending_requests: Vec::new(),
    }
  }

  /// Returns true if a save file is loaded and ready.
  pub fn is_ready(&self) -> bool {
    // current_save is set when initialization completes on both platforms
    self.save_name.is_some()
  }

  /// Returns the save name (without .save extension).
  pub fn save_name(&self) -> Option<&str> {
    self.save_name.as_deref()
  }

  /// Returns a reference to the loaded save, if any.
  pub fn world_save(&self) -> Option<&Arc<RwLock<WorldSave>>> {
    self.world_save.as_ref()
  }

  /// Returns a reference to the persistence backend.
  ///
  /// Returns None on WASM with IoDispatcher (I/O is in worker).
  pub fn backend(&self) -> Option<&dyn PersistenceBackend> {
    self.backend.as_ref().map(|b| &**b)
  }

  /// Saves to a named save file.
  ///
  /// If `name` differs from `current_save`, performs a copy-on-write:
  /// the current save file is copied to the new name before writing.
  ///
  /// Returns a handle that can be polled or awaited to check completion.
  ///
  /// # Example
  /// ```ignore
  /// fn exit_game_system(
  ///     mut persistence: ResMut<PersistenceControl>,
  ///     mut exit: EventWriter<AppExit>,
  /// ) {
  ///     // Save to the current save name
  ///     let handle = persistence.save("world");
  ///
  ///     // Or save to a new slot
  ///     let handle = persistence.save("backup");
  /// }
  /// ```
  pub fn save(&mut self, name: &str) -> PersistenceHandle {
    self.save_internal(name, true)
  }

  /// Saves only chunk data to a named save file (no pixel bodies).
  ///
  /// Faster than full save when pixel bodies haven't changed.
  pub fn save_chunks(&mut self, name: &str) -> PersistenceHandle {
    self.save_internal(name, false)
  }

  /// Internal helper for save operations.
  fn save_internal(&mut self, name: &str, include_bodies: bool) -> PersistenceHandle {
    let id = self.next_request_id;
    self.next_request_id += 1;

    let completed = Arc::new(AtomicBool::new(false));
    let request = PersistenceRequestInner {
      id,
      completed: completed.clone(),
      include_bodies,
      target_save: name.to_string(),
    };
    self.pending_requests.push(request);

    PersistenceHandle { id, completed }
  }

  /// Returns the file name for a named save.
  pub fn save_file_name(name: &str) -> String {
    format!("{}.save", name)
  }

  /// Lists all save files in the storage backend.
  pub fn list_saves(&self) -> io::Result<Vec<String>> {
    let backend = self
      .backend
      .as_ref()
      .ok_or_else(|| io::Error::new(io::ErrorKind::Unsupported, "no backend available"))?;
    backend.list_saves()
  }

  /// Deletes a save file.
  pub fn delete_save(&self, name: &str) -> io::Result<()> {
    let backend = self
      .backend
      .as_ref()
      .ok_or_else(|| io::Error::new(io::ErrorKind::Unsupported, "no backend available"))?;
    backend.delete_save(name)
  }

  /// Copies the current save to a new name (copy-on-write for "Save As").
  ///
  /// Returns an error if no save is loaded or no backend available.
  /// Updates the current save to point to the copied file.
  pub fn copy_to(&mut self, target_name: &str) -> io::Result<()> {
    let backend = self
      .backend
      .as_ref()
      .ok_or_else(|| io::Error::new(io::ErrorKind::Unsupported, "no backend available"))?;

    let save_arc = self
      .world_save
      .as_ref()
      .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no save loaded"))?;

    let mut save = save_arc
      .write()
      .map_err(|_| io::Error::new(io::ErrorKind::Other, "save lock poisoned"))?;

    let new_save = backend.save_copy(&mut save, target_name)?;

    // Replace the current save with the new one
    drop(save);
    self.world_save = Some(Arc::new(RwLock::new(new_save)));
    self.save_name = Some(target_name.to_string());

    Ok(())
  }
}

/// Internal representation of a pending persistence request.
pub(crate) struct PersistenceRequestInner {
  pub id: u64,
  pub completed: Arc<AtomicBool>,
  pub include_bodies: bool,
  /// Target save name for this request.
  pub target_save: String,
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
/// Alternative to using `PersistenceControl::request_save()`.
/// Useful when you need to trigger a save from a system that doesn't
/// have mutable access to `PersistenceControl`.
#[derive(bevy::prelude::Message, Clone, Debug, Default)]
pub struct RequestPersistence {
  /// Whether to include pixel bodies in the save.
  pub include_bodies: bool,
}

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
  /// Save name that was requested.
  pub save_name: String,
  /// World seed.
  pub world_seed: u64,
}

impl RequestPersistence {
  /// Creates a request for a full save (chunks + bodies).
  pub fn full() -> Self {
    Self {
      include_bodies: true,
    }
  }

  /// Creates a request for chunk-only save.
  pub fn chunks_only() -> Self {
    Self {
      include_bodies: false,
    }
  }
}
