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
use bevy_pixel_world::{
  FreshReseedAllChunks, MaterialSeeder, PersistenceControl, PersistenceHandle, ReloadAllChunks,
  SimulationState, UpdateSeeder,
};
#[cfg(feature = "editor")]
use bevy_yoleck::YoleckEditorLevelsDirectoryPath;
#[cfg(feature = "editor")]
use bevy_yoleck::prelude::YoleckSyncWithEditorState;
use bevy_yoleck::prelude::*;
use bevy_yoleck::vpeol::prelude::*;
pub use entities::{PlayerSpawnPoint, WorldConfigData};

/// Pending reseed after save completes.
///
/// When entering edit mode, we save first, then reseed. This resource holds
/// the save handle so we can poll for completion before reseeding.
#[cfg(feature = "editor")]
#[derive(Resource)]
struct PendingReseedAfterSave(PersistenceHandle);

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
      // Poll for save completion, then reseed
      app.add_systems(
        Update,
        poll_pending_reseed.run_if(in_state(GameMode::Editing)),
      );
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
  mut commands: Commands,
  simulation: Option<ResMut<SimulationState>>,
  mut persistence: Option<ResMut<PersistenceControl>>,
  brush: Option<ResMut<bevy_pixel_world::BrushState>>,
) {
  // Trigger save BEFORE disabling persistence.
  // Store the handle so we can reseed AFTER save completes.
  if let Some(ref mut pers) = persistence {
    if pers.is_active() {
      let handle = pers.save();
      commands.insert_resource(PendingReseedAfterSave(handle));
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

  info!("Edit mode: simulation paused, persistence disabled, brush disabled");
}

/// System: Polls for pending save completion, then reseeds with correct noise profile.
#[cfg(feature = "editor")]
fn poll_pending_reseed(
  mut commands: Commands,
  pending: Option<Res<PendingReseedAfterSave>>,
  profile: Res<noise::NoiseProfile>,
  mut update_seeder: bevy::ecs::message::MessageWriter<UpdateSeeder>,
  mut fresh_reseed: bevy::ecs::message::MessageWriter<FreshReseedAllChunks>,
) {
  let Some(pending) = pending else { return };

  if pending.0.is_complete() {
    // Update seeder from the current noise profile BEFORE reseeding.
    // This ensures chunks are seeded with the yoleck-saved noise config,
    // not the default MaterialSeeder::new(42) from world spawn.
    if let Some(seeder) = MaterialSeeder::from_encoded(&profile.ent, profile.world_seed) {
      let seeder = seeder.threshold(profile.threshold);
      update_seeder.write(UpdateSeeder {
        seeder: std::sync::Arc::new(seeder),
      });
      info!(
        "Updated seeder from noise profile: seed={}, threshold={}",
        profile.world_seed, profile.threshold
      );
    } else {
      warn!("Failed to create seeder from noise profile ENT");
    }

    fresh_reseed.write(FreshReseedAllChunks);
    commands.remove_resource::<PendingReseedAfterSave>();
    info!("Save complete, reseeding chunks with fresh procedural data");
  }
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
