mod graph;
mod time_series;

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
pub use graph::{TimeSeriesGraphConfig, time_series_graph};
pub use time_series::TimeSeries;

const SAMPLE_CAPACITY: usize = 300;

#[derive(Resource)]
pub struct FrameTimeMetrics {
  pub frame_time: TimeSeries,
  pub fps: TimeSeries,
}

impl Default for FrameTimeMetrics {
  fn default() -> Self {
    Self {
      frame_time: TimeSeries::new(SAMPLE_CAPACITY),
      fps: TimeSeries::new(SAMPLE_CAPACITY),
    }
  }
}

/// Metrics for pixel world simulation timing.
#[derive(Resource)]
pub struct SimulationMetrics {
  pub sim_time: TimeSeries,
  pub upload_time: TimeSeries,
}

impl Default for SimulationMetrics {
  fn default() -> Self {
    Self {
      sim_time: TimeSeries::new(SAMPLE_CAPACITY),
      upload_time: TimeSeries::new(SAMPLE_CAPACITY),
    }
  }
}

pub struct DiagnosticsPlugin;

impl Plugin for DiagnosticsPlugin {
  fn build(&self, app: &mut App) {
    if !app.is_plugin_added::<EguiPlugin>() {
      app.add_plugins(EguiPlugin::default());
    }
    app
      .init_resource::<FrameTimeMetrics>()
      .init_resource::<SimulationMetrics>()
      .add_systems(First, collect_frame_metrics)
      .add_systems(EguiPrimaryContextPass, render_diagnostics_ui);
  }
}

fn collect_frame_metrics(time: Res<Time>, mut metrics: ResMut<FrameTimeMetrics>) {
  let delta_secs = time.delta_secs();
  let frame_time_ms = delta_secs * 1000.0;
  let fps = if delta_secs > 0.0 {
    1.0 / delta_secs
  } else {
    0.0
  };

  metrics.frame_time.push(frame_time_ms);
  metrics.fps.push(fps);
}

fn render_diagnostics_ui(
  mut contexts: EguiContexts,
  mut metrics: ResMut<FrameTimeMetrics>,
  mut sim_metrics: ResMut<SimulationMetrics>,
) {
  let Ok(ctx) = contexts.ctx_mut() else {
    return;
  };
  egui::Window::new("Diagnostics")
    .anchor(egui::Align2::RIGHT_TOP, [-10.0, 10.0])
    .default_width(220.0)
    .show(ctx, |ui| {
      time_series_graph(
        ui,
        &mut metrics.frame_time,
        TimeSeriesGraphConfig {
          label: "Frame Time",
          unit: "ms",
          target_line: Some(16.67), // 60 FPS target
          ..Default::default()
        },
      );

      ui.add_space(4.0);

      time_series_graph(
        ui,
        &mut metrics.fps,
        TimeSeriesGraphConfig {
          label: "FPS",
          unit: "",
          line_color: egui::Color32::from_rgb(100, 150, 255),
          target_line: Some(60.0),
          ..Default::default()
        },
      );

      ui.add_space(4.0);

      time_series_graph(
        ui,
        &mut sim_metrics.sim_time,
        TimeSeriesGraphConfig {
          label: "Sim",
          unit: "ms",
          line_color: egui::Color32::from_rgb(255, 150, 100),
          ..Default::default()
        },
      );

      ui.add_space(4.0);

      time_series_graph(
        ui,
        &mut sim_metrics.upload_time,
        TimeSeriesGraphConfig {
          label: "Upload",
          unit: "ms",
          line_color: egui::Color32::from_rgb(200, 100, 255),
          ..Default::default()
        },
      );
    });
}
