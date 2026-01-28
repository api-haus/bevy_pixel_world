//! Persistence for visual debug settings.

use std::path::PathBuf;
use std::time::Duration;

use bevy::prelude::*;
// WASM compat: std::time::Instant panics on wasm32
use web_time::Instant;

use super::settings::VisualDebugSettings;

const SETTINGS_FILE: &str = "debug_settings.toml";
const DEBOUNCE_DURATION: Duration = Duration::from_millis(500);

/// Tracks when settings were last changed for debounced saving.
#[derive(Resource)]
pub struct SettingsPersistence {
  /// Last time settings were modified.
  last_change: Option<Instant>,
  /// Whether a save is pending.
  save_pending: bool,
  /// Path to settings file.
  settings_path: Option<PathBuf>,
}

impl Default for SettingsPersistence {
  fn default() -> Self {
    Self {
      last_change: None,
      save_pending: false,
      settings_path: get_settings_path(),
    }
  }
}

impl SettingsPersistence {
  /// Marks settings as changed, triggering a debounced save.
  pub fn mark_changed(&mut self) {
    self.last_change = Some(Instant::now());
    self.save_pending = true;
  }
}

/// Returns the path to the settings file.
fn get_settings_path() -> Option<PathBuf> {
  #[cfg(feature = "native")]
  {
    let data_dir = dirs::data_dir()?;
    let app_dir = data_dir.join("bevy_pixel_world");
    Some(app_dir.join(SETTINGS_FILE))
  }
  #[cfg(not(feature = "native"))]
  {
    None
  }
}

/// Loads settings from disk on startup.
pub fn load_settings(mut commands: Commands) {
  let settings = match get_settings_path() {
    Some(path) if path.exists() => match std::fs::read_to_string(&path) {
      Ok(contents) => match toml::from_str(&contents) {
        Ok(settings) => {
          info!("Loaded debug settings from {}", path.display());
          settings
        }
        Err(e) => {
          warn!("Failed to parse debug settings: {e}, using defaults");
          VisualDebugSettings::default()
        }
      },
      Err(e) => {
        warn!("Failed to read debug settings: {e}, using defaults");
        VisualDebugSettings::default()
      }
    },
    _ => VisualDebugSettings::default(),
  };

  commands.insert_resource(settings);
  commands.insert_resource(SettingsPersistence::default());
}

/// Saves settings to disk when changed (debounced).
pub fn save_settings(
  settings: Res<VisualDebugSettings>,
  mut persistence: ResMut<SettingsPersistence>,
) {
  if !persistence.save_pending {
    return;
  }

  let Some(last_change) = persistence.last_change else {
    return;
  };

  // Debounce: wait for changes to settle
  if last_change.elapsed() < DEBOUNCE_DURATION {
    return;
  }

  persistence.save_pending = false;

  let Some(path) = &persistence.settings_path else {
    return;
  };

  // Ensure parent directory exists
  if let Some(parent) = path.parent()
    && let Err(e) = std::fs::create_dir_all(parent)
  {
    warn!("Failed to create settings directory: {e}");
    return;
  }

  match toml::to_string_pretty(&*settings) {
    Ok(contents) => {
      if let Err(e) = std::fs::write(path, contents) {
        warn!("Failed to write debug settings: {e}");
      } else {
        debug!("Saved debug settings to {}", path.display());
      }
    }
    Err(e) => {
      warn!("Failed to serialize debug settings: {e}");
    }
  }
}
