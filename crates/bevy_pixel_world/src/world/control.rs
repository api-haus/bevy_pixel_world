//! World control APIs - pause/resume simulation and on-demand persistence.
//!
//! Provides public APIs for:
//! - Pausing/resuming world simulation and physics
//! - Triggering on-demand persistence with completion notification
//! - Configuring periodic auto-save

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use bevy::prelude::*;

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

/// Configuration for periodic auto-save.
#[derive(Clone, Debug)]
pub struct AutoSaveConfig {
  /// Whether auto-save is enabled.
  pub enabled: bool,
  /// Interval between auto-saves.
  pub interval: Duration,
}

impl Default for AutoSaveConfig {
  fn default() -> Self {
    Self {
      enabled: true,
      interval: Duration::from_secs(5), // Auto-save every 5 seconds
    }
  }
}

impl AutoSaveConfig {
  /// Creates a new auto-save config with the given interval.
  pub fn with_interval(interval: Duration) -> Self {
    Self {
      enabled: true,
      interval,
    }
  }

  /// Disables auto-save.
  pub fn disabled() -> Self {
    Self {
      enabled: false,
      ..Default::default()
    }
  }
}

/// Resource for persistence control and timing.
#[derive(Resource)]
pub struct PersistenceControl {
  /// Auto-save configuration.
  pub auto_save: AutoSaveConfig,
  /// Time since last auto-save.
  pub(crate) time_since_save: Duration,
  /// Counter for generating unique request IDs.
  next_request_id: u64,
  /// Pending persistence requests.
  pub(crate) pending_requests: Vec<PersistenceRequestInner>,
}

impl Default for PersistenceControl {
  fn default() -> Self {
    Self {
      auto_save: AutoSaveConfig::default(),
      time_since_save: Duration::ZERO,
      next_request_id: 1,
      pending_requests: Vec::new(),
    }
  }
}

impl PersistenceControl {
  /// Creates a new persistence control with the given auto-save config.
  pub fn new(auto_save: AutoSaveConfig) -> Self {
    Self {
      auto_save,
      ..Default::default()
    }
  }

  /// Requests an on-demand save operation.
  ///
  /// Returns a handle that can be polled or awaited to check completion.
  ///
  /// # Example
  /// ```ignore
  /// fn exit_game_system(
  ///     mut persistence: ResMut<PersistenceControl>,
  ///     mut exit: EventWriter<AppExit>,
  /// ) {
  ///     // Request save and store handle
  ///     let save_handle = persistence.request_save();
  ///
  ///     // In a real app, you'd poll this in another system
  ///     // and exit when complete
  /// }
  /// ```
  pub fn request_save(&mut self) -> PersistenceHandle {
    let id = self.next_request_id;
    self.next_request_id += 1;

    let completed = Arc::new(AtomicBool::new(false));
    let request = PersistenceRequestInner {
      id,
      completed: completed.clone(),
      include_bodies: true,
    };
    self.pending_requests.push(request);

    PersistenceHandle { id, completed }
  }

  /// Requests a save of only chunk data (no pixel bodies).
  ///
  /// Faster than full save when pixel bodies haven't changed.
  pub fn request_chunk_save(&mut self) -> PersistenceHandle {
    let id = self.next_request_id;
    self.next_request_id += 1;

    let completed = Arc::new(AtomicBool::new(false));
    let request = PersistenceRequestInner {
      id,
      completed: completed.clone(),
      include_bodies: false,
    };
    self.pending_requests.push(request);

    PersistenceHandle { id, completed }
  }

  /// Resets the auto-save timer.
  ///
  /// Call this after completing a save to restart the countdown.
  pub fn reset_auto_save_timer(&mut self) {
    self.time_since_save = Duration::ZERO;
  }
}

/// Internal representation of a pending persistence request.
pub(crate) struct PersistenceRequestInner {
  pub id: u64,
  pub completed: Arc<AtomicBool>,
  pub include_bodies: bool,
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
