//! World control APIs - pause/resume simulation and on-demand persistence.
//!
//! Provides public APIs for:
//! - Pausing/resuming world simulation and physics
//! - Triggering on-demand persistence with completion notification
//! - Managing named saves

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;

use crate::persistence::backend::StorageFs;

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
  /// Storage filesystem backend.
  pub(crate) fs: Box<dyn StorageFs>,
  /// Currently loaded save name.
  pub(crate) current_save: String,
  /// Counter for generating unique request IDs.
  next_request_id: u64,
  /// Pending persistence requests.
  pub(crate) pending_requests: Vec<PersistenceRequestInner>,
}

impl PersistenceControl {
  /// Creates a new persistence control with the given storage backend and save
  /// name.
  pub fn new(fs: Box<dyn StorageFs>, current_save: String) -> Self {
    Self {
      fs,
      current_save,
      next_request_id: 1,
      pending_requests: Vec::new(),
    }
  }

  /// Returns the currently loaded save name.
  pub fn current_save(&self) -> &str {
    &self.current_save
  }

  /// Returns a reference to the storage filesystem backend.
  pub fn fs(&self) -> &dyn StorageFs {
    &*self.fs
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
    let all_files = crate::persistence::block_on(self.fs.list()).map_err(io::Error::from)?;

    let mut saves: Vec<String> = all_files
      .into_iter()
      .filter_map(|name| name.strip_suffix(".save").map(String::from))
      .collect();

    saves.sort();
    Ok(saves)
  }

  /// Deletes a save file.
  pub fn delete_save(&self, name: &str) -> io::Result<()> {
    let file_name = Self::save_file_name(name);
    crate::persistence::block_on(self.fs.delete(&file_name)).map_err(io::Error::from)
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
