//! Persistent console command history.
//!
//! Loads history from disk on startup and saves after commands are executed.
//! Uses debounced saving to avoid excessive I/O.

use std::path::PathBuf;
use std::time::Duration;

use bevy::prelude::*;
use bevy_console::ConsoleState;
use serde::{Deserialize, Serialize};
// WASM compat: std::time::Instant panics on wasm32
use web_time::Instant;

const HISTORY_FILE: &str = "console_history.toml";
const DEBOUNCE_DURATION: Duration = Duration::from_millis(500);
const MAX_HISTORY_ENTRIES: usize = 100;

/// Serializable console history.
#[derive(Serialize, Deserialize, Default)]
struct ConsoleHistory {
  /// Command history entries (most recent last).
  commands: Vec<String>,
}

/// Tracks console history persistence state.
#[derive(Resource)]
pub struct HistoryPersistence {
  /// Last known history length (to detect changes).
  last_history_len: usize,
  /// Time of last detected change.
  last_change: Option<Instant>,
  /// Whether a save is pending.
  save_pending: bool,
  /// Path to history file.
  history_path: Option<PathBuf>,
}

impl Default for HistoryPersistence {
  fn default() -> Self {
    Self {
      last_history_len: 1, // ConsoleState starts with one empty entry
      last_change: None,
      save_pending: false,
      history_path: get_history_path(),
    }
  }
}

/// Returns the path to the history file.
fn get_history_path() -> Option<PathBuf> {
  #[cfg(not(target_family = "wasm"))]
  {
    let data_dir = dirs::data_dir()?;
    let app_dir = data_dir.join("bevy_pixel_world");
    Some(app_dir.join(HISTORY_FILE))
  }
  #[cfg(target_family = "wasm")]
  {
    None
  }
}

/// Loads history from disk and injects into ConsoleState.
///
/// Must run after ConsolePlugin initializes ConsoleState.
pub fn load_history(mut console_state: ResMut<ConsoleState>) {
  let Some(path) = get_history_path() else {
    return;
  };

  if !path.exists() {
    return;
  }

  let Ok(contents) = std::fs::read_to_string(&path) else {
    warn!("Failed to read console history from {}", path.display());
    return;
  };

  let history: ConsoleHistory = match toml::from_str(&contents) {
    Ok(h) => h,
    Err(e) => {
      warn!("Failed to parse console history: {e}");
      return;
    }
  };

  // ConsoleState.history is VecDeque with index 0 = current (empty) buffer
  // Inject loaded history after index 0
  for cmd in history.commands.into_iter().rev() {
    if !cmd.is_empty() {
      console_state.history.push_back(cmd);
    }
  }

  debug!(
    "Loaded {} history entries from {}",
    console_state.history.len() - 1,
    path.display()
  );
}

/// Initializes the history persistence resource.
pub fn init_persistence(mut commands: Commands, console_state: Res<ConsoleState>) {
  commands.insert_resource(HistoryPersistence {
    last_history_len: console_state.history.len(),
    ..default()
  });
}

/// Detects history changes and triggers debounced saving.
pub fn track_history_changes(
  console_state: Res<ConsoleState>,
  mut persistence: ResMut<HistoryPersistence>,
) {
  let current_len = console_state.history.len();

  if current_len != persistence.last_history_len {
    persistence.last_history_len = current_len;
    persistence.last_change = Some(Instant::now());
    persistence.save_pending = true;
  }
}

/// Saves history to disk (debounced).
pub fn save_history(console_state: Res<ConsoleState>, mut persistence: ResMut<HistoryPersistence>) {
  if !persistence.save_pending {
    return;
  }

  let Some(last_change) = persistence.last_change else {
    return;
  };

  if last_change.elapsed() < DEBOUNCE_DURATION {
    return;
  }

  persistence.save_pending = false;

  let Some(path) = &persistence.history_path else {
    return;
  };

  // Ensure parent directory exists
  if let Some(parent) = path.parent()
    && let Err(e) = std::fs::create_dir_all(parent)
  {
    warn!("Failed to create history directory: {e}");
    return;
  }

  // Extract commands from history (skip index 0 which is current buffer)
  let commands: Vec<String> = console_state
    .history
    .iter()
    .skip(1)
    .take(MAX_HISTORY_ENTRIES)
    .cloned()
    .collect();

  let history = ConsoleHistory { commands };

  match toml::to_string_pretty(&history) {
    Ok(contents) => {
      if let Err(e) = std::fs::write(path, contents) {
        warn!("Failed to write console history: {e}");
      } else {
        debug!("Saved console history to {}", path.display());
      }
    }
    Err(e) => {
      warn!("Failed to serialize console history: {e}");
    }
  }
}
