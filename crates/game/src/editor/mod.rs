#[cfg(all(feature = "editor", target_family = "wasm"))]
compile_error!("Editor feature is not supported on WASM");

#[cfg(feature = "editor")]
mod actions;
mod entities;
#[cfg(feature = "editor")]
mod noise;
#[cfg(feature = "editor")]
mod ui;

use bevy::prelude::*;
#[cfg(feature = "editor")]
use bevy_pixel_world::{PersistenceControl, ReloadAllChunks, SimulationState};
#[cfg(feature = "editor")]
use bevy_yoleck::YoleckEditorLevelsDirectoryPath;
#[cfg(feature = "editor")]
use bevy_yoleck::prelude::YoleckSyncWithEditorState;
use bevy_yoleck::prelude::*;
use bevy_yoleck::vpeol::prelude::*;
pub use entities::{PlayerSpawnPoint, WorldConfigData};

/// Game mode state controlling editor vs play mode.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameMode {
  /// Level editor active, entities selectable/draggable, player not spawned.
  #[cfg_attr(not(feature = "editor"), allow(dead_code))]
  Editing,
  /// Game active, player spawned and controllable.
  #[default]
  Playing,
}

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
  fn build(&self, app: &mut App) {
    // Add yoleck plugins - either editor or game mode, not both
    #[cfg(feature = "editor")]
    {
      if !app.is_plugin_added::<bevy_egui::EguiPlugin>() {
        app.add_plugins(bevy_egui::EguiPlugin::default());
      }
      app.add_plugins(YoleckPluginForEditor);
      app.add_plugins(Vpeol2dPluginForEditor);

      // Sync GameMode with YoleckEditorState - yoleck drives the state
      app.add_plugins(YoleckSyncWithEditorState {
        when_editor: GameMode::Editing,
        when_game: GameMode::Playing,
      });

      // Configure level directory
      app.insert_resource(YoleckEditorLevelsDirectoryPath(
        std::path::Path::new("assets").join("levels"),
      ));

      // Register edit systems (only for editor)
      entities::register_edit_systems(app);

      // Add editor UI (runs after egui context is ready)
      ui::configure_system_sets(app);

      // Noise profile panel with NoiseTool IPC
      noise::setup(app);

      // Keyboard shortcuts for toggling editor/play state
      app.add_systems(
        Update,
        (
          actions::editor_keyboard_shortcuts,
          actions::editor_camera_pan.run_if(in_state(YoleckEditorState::EditorActive)),
        ),
      );

      // Sync simulation/persistence with edit mode
      app.add_systems(OnEnter(GameMode::Editing), on_enter_editing);
      app.add_systems(OnEnter(GameMode::Playing), on_enter_playing);
    }
    #[cfg(not(feature = "editor"))]
    {
      // Game mode state (in non-editor builds, we manage it ourselves)
      app.init_state::<GameMode>();
      app.add_plugins(YoleckPluginForGame);
      app.add_plugins(Vpeol2dPluginForGame);
    }

    // Register entity types - needed for both editor and game
    entities::register_entity_types(app);

    // Load level from file
    app.add_systems(Startup, load_default_level);
  }
}

/// Loads the default level from the .yol file.
fn load_default_level(mut commands: Commands, asset_server: Res<AssetServer>) {
  debug!("Loading default level from file");
  commands.spawn(YoleckLoadLevel(asset_server.load("levels/default.yol")));
}

#[cfg(feature = "editor")]
fn on_enter_editing(
  simulation: Option<ResMut<SimulationState>>,
  mut persistence: Option<ResMut<PersistenceControl>>,
  brush: Option<ResMut<bevy_pixel_world::BrushState>>,
) {
  // Trigger save BEFORE disabling persistence.
  // This adds to pending_requests which will be processed by Update systems
  // even after persistence is disabled (process_pending_save_requests doesn't
  // check is_enabled). Chunks stay in Active state so needs_save() returns true.
  // When entering play mode, ReloadAllChunks will load from disk.
  if let Some(ref mut pers) = persistence {
    if pers.is_active() {
      let _handle = pers.save();
      info!("Triggered save before entering edit mode");
    }
  }

  if let Some(mut sim) = simulation {
    sim.pause();
  }
  if let Some(mut pers) = persistence {
    pers.disable();
  }
  if let Some(mut brush) = brush {
    brush.enabled = false;
  }
  // Note: We intentionally do NOT reseed here. The save triggered above needs
  // chunks to stay in Active state so needs_save() returns true. When re-entering
  // play mode, ReloadAllChunks will load the saved state from disk.
  info!("Edit mode: simulation paused, persistence disabled, brush disabled");
}

#[cfg(feature = "editor")]
fn on_enter_playing(
  simulation: Option<ResMut<SimulationState>>,
  persistence: Option<ResMut<PersistenceControl>>,
  brush: Option<ResMut<bevy_pixel_world::BrushState>>,
  mut reload: bevy::ecs::message::MessageWriter<ReloadAllChunks>,
) {
  if let Some(mut pers) = persistence {
    pers.enable();
  }
  if let Some(mut sim) = simulation {
    sim.resume();
  }
  if let Some(mut brush) = brush {
    brush.enabled = true;
  }
  // Reload chunks from disk to restore persisted state
  reload.write(ReloadAllChunks);
  info!("Play mode: simulation resumed, persistence enabled, brush enabled, reloading from disk");
}
