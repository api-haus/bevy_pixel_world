use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use bevy_pixel_world::PixelWorld;
use bevy_pixel_world::pixel_body::{LastBlitTransform, PixelBody};

use crate::state::*;
use crate::test_phases::count_body_pixels;

pub fn diagnostic_ui(
  mut contexts: EguiContexts,
  state: Res<DebugState>,
  bodies: Query<(
    Entity,
    &PixelBody,
    &GlobalTransform,
    Option<&LastBlitTransform>,
  )>,
  worlds: Query<&mut PixelWorld>,
) {
  let Ok(ctx) = contexts.ctx_mut() else {
    return;
  };

  let body_count = bodies.iter().count();
  let total_solid: usize = bodies.iter().map(|(_, b, _, _)| b.solid_count()).sum();
  let world_body_pixels = count_body_pixels(&worlds);

  egui::SidePanel::left("debug_panel")
    .resizable(true)
    .default_width(320.0)
    .show(ctx, |ui| {
      ui.heading("Pixel Body Erasure Test");

      ui.separator();
      ui.label("Controls:");
      ui.label("  1 - Run stability test (spawn only)");
      ui.label("  2 - Run erasure test (spawn + erase)");
      ui.label("  3 - Run repositioning test (spawn + scroll)");
      ui.label("  Space - Manual spawn");
      ui.label("  E - Toggle manual erase sweep");
      ui.label("  WASD - Move camera");

      ui.separator();

      let (phase_text, phase_color) = match state.phase {
        TestPhase::Idle => ("IDLE", egui::Color32::GRAY),
        TestPhase::Spawning => ("SPAWNING", egui::Color32::YELLOW),
        TestPhase::Settling => ("SETTLING", egui::Color32::LIGHT_BLUE),
        TestPhase::Erasing => ("ERASING", egui::Color32::ORANGE),
        TestPhase::ScrollingUp => ("SCROLLING UP", egui::Color32::KHAKI),
        TestPhase::WaitingUp => ("WAITING UP", egui::Color32::LIGHT_BLUE),
        TestPhase::ScrollingDown => ("SCROLLING DOWN", egui::Color32::KHAKI),
        TestPhase::Verifying => ("VERIFYING", egui::Color32::LIGHT_BLUE),
        TestPhase::Done => {
          if state.test_passed == Some(true) {
            ("PASSED", egui::Color32::GREEN)
          } else if state.test_passed == Some(false) {
            ("FAILED", egui::Color32::RED)
          } else {
            ("DONE", egui::Color32::GRAY)
          }
        }
      };
      ui.horizontal(|ui| {
        ui.label("Phase:");
        ui.colored_label(phase_color, phase_text);
      });

      if let Some(test_type) = state.test_type {
        ui.label(format!("Test: {:?}", test_type));
      }

      ui.separator();
      ui.heading("Status");

      ui.label(format!("Bodies spawned: {}", state.bodies_spawned));
      ui.label(format!("Bodies alive: {}", body_count));
      ui.label(format!("Total solid pixels: {}", total_solid));
      ui.label(format!("World PIXEL_BODY flags: {}", world_body_pixels));

      if matches!(
        state.phase,
        TestPhase::Settling | TestPhase::Verifying | TestPhase::WaitingUp
      ) {
        ui.label(format!("Frame: {}", state.frame_counter));
      }

      ui.separator();
      ui.heading("Bodies");

      egui::ScrollArea::vertical()
        .id_salt("bodies_scroll")
        .max_height(150.0)
        .show(ui, |ui| {
          for (entity, body, transform, last_blit) in bodies.iter() {
            let pos = transform.translation();
            let solid = body.solid_count();
            let has_blit = last_blit.and_then(|b| b.transform.as_ref()).is_some();

            ui.horizontal(|ui| {
              ui.label(format!(
                "{:?}: ({:.0},{:.0}) s={} b={}",
                entity,
                pos.x,
                pos.y,
                solid,
                if has_blit { "Y" } else { "N" }
              ));
            });
          }
        });

      ui.separator();
      ui.heading("Log");

      egui::ScrollArea::vertical()
        .id_salt("log_scroll")
        .max_height(200.0)
        .stick_to_bottom(true)
        .show(ui, |ui| {
          for line in &state.log {
            let color = if line.contains("PASSED") {
              egui::Color32::GREEN
            } else if line.contains("FAILED") {
              egui::Color32::RED
            } else if line.starts_with("===") {
              egui::Color32::WHITE
            } else {
              egui::Color32::LIGHT_GRAY
            };
            ui.colored_label(color, line);
          }
        });
    });
}
