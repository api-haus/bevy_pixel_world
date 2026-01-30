//! Noise profile editor panel with NoiseTool IPC integration.

use bevy::prelude::*;
use bevy_egui::{EguiPrimaryContextPass, egui};
use noise_ipc::NoiseIpc;

/// Current noise profile being edited.
#[derive(Resource)]
pub struct NoiseProfile {
  /// Encoded node tree (ENT) string.
  pub ent: String,
  /// Whether the ENT has been modified since last save.
  pub dirty: bool,
}

impl Default for NoiseProfile {
  fn default() -> Self {
    // Default to simplex noise preset
    Self {
      ent: bevy_pixel_world::noise_presets::SIMPLEX.to_string(),
      dirty: false,
    }
  }
}

/// Optional IPC connection to NoiseTool.
#[derive(Resource, Default)]
pub struct NoiseIpcConnection {
  pub client: Option<NoiseIpc>,
}

pub fn setup(app: &mut App) {
  app.init_resource::<NoiseProfile>();
  app.init_resource::<NoiseIpcConnection>();
  app.add_systems(EguiPrimaryContextPass, noise_panel_system);
}

fn noise_panel_system(
  mut egui_ctx: bevy_egui::EguiContexts,
  mut profile: ResMut<NoiseProfile>,
  mut ipc: ResMut<NoiseIpcConnection>,
) {
  // Poll IPC for updates from NoiseTool
  if let Some(client) = &mut ipc.client {
    if let Some(new_ent) = client.poll() {
      if new_ent != profile.ent {
        profile.ent = new_ent;
        profile.dirty = true;
        info!("Received ENT update from NoiseTool");
      }
    }
  }

  let Ok(ctx) = egui_ctx.ctx_mut() else {
    return;
  };

  egui::Window::new("Noise Profile")
    .default_open(false)
    .show(ctx, |ui| {
      ui.horizontal(|ui| {
        ui.label("ENT:");
        let response = ui.text_edit_singleline(&mut profile.ent);
        if response.changed() {
          profile.dirty = true;
        }
      });

      ui.horizontal(|ui| {
        if ui.button("Edit in NoiseTool").clicked() {
          open_in_noise_tool(&mut ipc.client, &profile.ent);
        }

        if profile.dirty {
          ui.label(egui::RichText::new("(modified)").italics());
        }
      });
    });
}

fn open_in_noise_tool(ipc: &mut Option<NoiseIpc>, ent: &str) {
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
    let noise_tool_path = "./vendor/FastNoise2/build/Release/bin/NodeEditor";
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
    match NoiseIpc::new() {
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
