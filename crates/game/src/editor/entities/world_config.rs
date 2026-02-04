//! Level-wide world configuration entity.
//!
//! Stores seeder parameters (noise ENT, world seed, threshold) in yoleck
//! levels. Each level should have exactly one WorldConfig entity.

use bevy::prelude::*;
use bevy_yoleck::prelude::*;
use serde::{Deserialize, Serialize};

/// Yoleck component for level-wide world configuration.
///
/// Stores parameters used to configure the `MaterialSeeder` for procedural
/// terrain generation. Changes to these values trigger world regeneration.
#[derive(Clone, PartialEq, Serialize, Deserialize, Component, YoleckComponent)]
pub struct WorldConfigData {
  /// Encoded node tree (ENT) string for FastNoise2.
  pub noise_ent: String,
  /// World seed for procedural generation.
  pub world_seed: i32,
  /// Noise threshold for solid/void boundary.
  pub threshold: f32,
}

impl Default for WorldConfigData {
  fn default() -> Self {
    Self {
      noise_ent: crate::pixel_world::noise_presets::SIMPLEX.to_string(),
      world_seed: 42,
      threshold: 0.0,
    }
  }
}

pub fn register(app: &mut App) {
  app.add_yoleck_entity_type(YoleckEntityType::new("WorldConfig").with::<WorldConfigData>());
  app.add_systems(YoleckSchedule::Populate, populate_world_config);
}

#[cfg(feature = "editor")]
pub fn register_edit_systems(app: &mut App) {
  app.add_yoleck_edit_system(edit_world_config);
}

#[cfg(feature = "editor")]
fn edit_world_config(
  mut ui: ResMut<YoleckUi>,
  mut edit: YoleckEdit<&mut WorldConfigData>,
  mut ipc: ResMut<crate::editor::noise::NoiseIpcConnection>,
  mut profile: ResMut<crate::editor::noise::NoiseProfile>,
) {
  use bevy_egui::egui;

  let Ok(mut data) = edit.single_mut() else {
    return;
  };

  // Track if any value changed
  let mut changed = false;

  ui.horizontal(|ui| {
    ui.label("Seed:");
    if ui.add(egui::DragValue::new(&mut data.world_seed)).changed() {
      changed = true;
    }
  });

  ui.horizontal(|ui| {
    ui.label("Threshold:");
    if ui
      .add(egui::DragValue::new(&mut data.threshold).speed(0.01))
      .changed()
    {
      changed = true;
    }
  });

  ui.horizontal(|ui| {
    ui.label("ENT:");
    if ui.text_edit_singleline(&mut data.noise_ent).changed() {
      changed = true;
    }
  });

  // Sync changes to NoiseProfile (triggers seeder update)
  if changed {
    profile.ent = data.noise_ent.clone();
    profile.world_seed = data.world_seed;
    profile.threshold = data.threshold;
    profile.dirty = true;
  }

  ui.separator();

  if ui.button("Edit in NoiseTool").clicked() {
    open_in_noise_tool(&mut ipc.client, &data.noise_ent);
  }

  ui.label("Opens external FastNoise2 node editor");
}

/// Populate system - WorldConfig is a metadata-only entity (no visuals).
fn populate_world_config(mut populate: YoleckPopulate<&WorldConfigData>) {
  populate.populate(|_ctx, _cmd, _data| {
    // WorldConfig has no visual representation - it's purely metadata.
  });
}

#[cfg(feature = "editor")]
fn open_in_noise_tool(ipc: &mut Option<noise_ipc::NoiseIpc>, ent: &str) {
  // Check if NoiseTool is already running
  let already_running = std::process::Command::new("pgrep")
    .arg("-x")
    .arg("NodeEditor")
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .status()
    .map(|s| s.success())
    .unwrap_or(false);

  // Launch NoiseTool if not already running
  if !already_running {
    let noise_tool_path = "../../vendor/FastNoise2/build/Release/bin/NodeEditor";
    match std::process::Command::new(noise_tool_path).spawn() {
      Ok(_) => info!("Launched NoiseTool"),
      Err(e) => {
        error!("Failed to launch NoiseTool: {}", e);
        return;
      }
    }
    // Give NoiseTool time to start and create shared memory
    std::thread::sleep(std::time::Duration::from_millis(500));
    // Reset IPC connection since we just launched NoiseTool
    *ipc = None;
  }

  // Connect to IPC (after NoiseTool is running)
  if ipc.is_none() {
    match noise_ipc::NoiseIpc::new() {
      Ok(client) => {
        *ipc = Some(client);
        info!("Connected to NoiseTool IPC");
      }
      Err(e) => {
        error!("Failed to connect to NoiseTool IPC: {}", e);
        return;
      }
    }
  }

  // Send import command
  if let Some(client) = ipc {
    client.send_import(ent);
    info!("Sent ENT to NoiseTool for editing");
  }
}
