use std::f32::consts::{FRAC_PI_2, TAU};

use bevy::prelude::*;
use bevy_egui::{EguiContext, egui};

use crate::time_of_day::TimeOfDay;

pub struct ClockWidgetPlugin;

/// Marker resource indicating egui is ready for UI drawing
#[derive(Resource, Default)]
struct EguiReady(u32);

impl Plugin for ClockWidgetPlugin {
  fn build(&self, app: &mut App) {
    if !app.is_plugin_added::<bevy_egui::EguiPlugin>() {
      app.add_plugins(bevy_egui::EguiPlugin::default());
    }
    app.init_resource::<EguiReady>();
    app.add_systems(Update, draw_clock_widget);
  }
}

fn draw_clock_widget(
  mut egui_ctx: Query<&mut EguiContext>,
  time_of_day: Option<Res<TimeOfDay>>,
  mut ready: ResMut<EguiReady>,
) {
  // Skip early frames to allow egui to fully initialize
  if ready.0 < 5 {
    ready.0 += 1;
    return;
  }

  let Some(time) = time_of_day else { return };
  let Ok(ctx) = egui_ctx.single_mut() else {
    return;
  };
  let ctx: &egui::Context = ctx.into_inner().get_mut();

  egui::Area::new(egui::Id::new("time_clock"))
    .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-10.0, -10.0))
    .interactable(false)
    .show(ctx, |ui| {
      let size = 50.0;
      let (response, painter) = ui.allocate_painter(egui::vec2(size, size), egui::Sense::hover());

      let center = response.rect.center();
      let radius = size / 2.0 - 2.0;

      // Background circle
      painter.circle_filled(center, radius, egui::Color32::from_black_alpha(128));

      // Hour arrow: 0h = up (north), 6h = right (east), 12h = down (south), 18h =
      // left (west)
      let angle = (time.hour / 24.0) * TAU - FRAC_PI_2;
      let arrow_end = center + egui::vec2(angle.cos(), angle.sin()) * (radius - 4.0);
      painter.line_segment(
        [center, arrow_end],
        egui::Stroke::new(2.0, egui::Color32::WHITE),
      );
    });
}
