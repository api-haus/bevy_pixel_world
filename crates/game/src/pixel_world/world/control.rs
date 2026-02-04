//! World control APIs - pause/resume simulation and on-demand persistence.
//!
//! Provides public APIs for:
//! - Pausing/resuming world simulation and physics
//! - Triggering on-demand persistence with completion notification

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;

use crate::pixel_world::seeding::ChunkSeeder;

/// Controls whether world simulation is running or paused.
///
/// When paused:
/// - Cellular automata simulation stops
/// - Pixel body physics is disabled
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
/// path. Can be disabled at runtime for level editor mode.
#[derive(Resource)]
pub struct PersistenceControl {
  /// Whether persistence is enabled. When disabled, no I/O occurs.
  enabled: bool,
  /// Current save file path.
  pub(crate) current_path: Option<PathBuf>,
  /// Counter for generating unique request IDs.
  next_request_id: u64,
  /// Pending persistence requests.
  pub(crate) pending_requests: Vec<PersistenceRequestInner>,
}

impl PersistenceControl {
  /// Creates a persistence control that tracks the save file path.
  ///
  /// All I/O is handled by IoDispatcher on both native and WASM platforms.
  /// Persistence is enabled by default.
  pub fn with_path_only(path: PathBuf) -> Self {
    Self {
      enabled: true,
      current_path: Some(path),
      next_request_id: 1,
      pending_requests: Vec::new(),
    }
  }

  /// Disables persistence. No save/load I/O will occur while disabled.
  ///
  /// Use this for level editor mode to prevent player state from being saved.
  pub fn disable(&mut self) {
    self.enabled = false;
  }

  /// Enables persistence. Save/load I/O will resume.
  pub fn enable(&mut self) {
    self.enabled = true;
  }

  /// Returns true if persistence is enabled.
  ///
  /// When disabled, all persistence systems skip their work.
  pub fn is_enabled(&self) -> bool {
    self.enabled
  }

  /// Returns true if persistence is enabled and a save file is open.
  ///
  /// Check this before calling save methods.
  pub fn is_active(&self) -> bool {
    self.enabled && self.current_path.is_some()
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
  fn save_internal(&mut self, _target_path: Option<PathBuf>) -> PersistenceHandle {
    // TODO: target_path for copy-on-write requires IoDispatcher CopyTo command
    let id = self.next_request_id;
    self.next_request_id += 1;

    let completed = Arc::new(AtomicBool::new(false));
    let request = PersistenceRequestInner {
      id,
      completed: completed.clone(),
    };
    self.pending_requests.push(request);

    PersistenceHandle { id, completed }
  }
}

/// Internal representation of a pending persistence request.
#[allow(dead_code)] // Fields used on native but not WASM
pub(crate) struct PersistenceRequestInner {
  pub id: u64,
  pub completed: Arc<AtomicBool>,
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

/// Event to trigger re-seeding of all active chunks.
///
/// When sent, all chunks in the `Active` lifecycle state transition back to
/// `Seeding`, causing them to regenerate with the current noise profile.
/// Any cached persistence data is cleared first.
///
/// Use this for level editor mode when the noise profile changes.
#[derive(bevy::prelude::Message)]
pub struct ReseedAllChunks;

/// Message to update the world seeder and regenerate all chunks.
///
/// When sent, the seeder is replaced on all `PixelWorld` instances,
/// then `ReseedAllChunks` is triggered to regenerate with the new seeder.
///
/// Use this when the noise profile changes in the editor.
#[derive(bevy::prelude::Message)]
pub struct UpdateSeeder {
  /// The new seeder to use for chunk generation.
  pub seeder: Arc<dyn ChunkSeeder + Send + Sync>,
}

/// Message to reload all chunks from disk.
///
/// When sent, all chunks in the `Active` lifecycle state transition back to
/// `Loading`, causing them to re-fetch data from the save file. Unsaved
/// in-memory changes are discarded.
///
/// Use this to revert to the last saved state.
#[derive(bevy::prelude::Message)]
pub struct ReloadAllChunks;

/// Message to clear the current save file and regenerate.
///
/// When sent, the save file is deleted and reinitialized with an empty state.
/// Should be followed by `ReseedAllChunks` to regenerate from procedural noise.
#[derive(bevy::prelude::Message)]
pub struct ClearPersistence;

/// Message to reseed all chunks with fresh procedural data.
///
/// Unlike `ReseedAllChunks` (which may be used after `UpdateSeeder`), this
/// ONLY transitions Active chunks to Seeding without updating the seeder.
/// Use for edit mode transitions where you want fresh procedural data.
#[derive(bevy::prelude::Message)]
pub struct FreshReseedAllChunks;
