mod graph;
mod profiler;
mod time_series;

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
pub use graph::{TimeSeriesGraphConfig, time_series_graph};
pub use profiler::{ProfilerMetrics, profile};
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

/// Metrics for collision mesh generation timing.
#[derive(Resource)]
pub struct CollisionMetrics {
  /// Time spent dispatching collision tasks (per frame).
  pub dispatch_time: TimeSeries,
  /// Total time spent generating collision meshes (summed from completed
  /// tasks).
  pub generation_time: TimeSeries,
  /// Number of tasks completed this frame.
  pub tasks_completed: TimeSeries,
}

impl Default for CollisionMetrics {
  fn default() -> Self {
    Self {
      dispatch_time: TimeSeries::new(SAMPLE_CAPACITY),
      generation_time: TimeSeries::new(SAMPLE_CAPACITY),
      tasks_completed: TimeSeries::new(SAMPLE_CAPACITY),
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
      .init_resource::<CollisionMetrics>()
      .init_resource::<ProfilerMetrics>()
      .add_systems(
        First,
        (profiler::aggregate_profiler_samples, collect_frame_metrics).chain(),
      )
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
  mut collision_metrics: ResMut<CollisionMetrics>,
  profiler_metrics: Res<ProfilerMetrics>,
) {
  let Ok(ctx) = contexts.ctx_mut() else {
    return;
  };
  egui::Window::new("Diagnostics")
    .anchor(egui::Align2::RIGHT_TOP, [-10.0, 10.0])
    .default_width(220.0)
    .title_bar(false)
    .resizable(false)
    .movable(false)
    .frame(egui::Frame::NONE.fill(egui::Color32::from_rgba_unmultiplied(20, 20, 25, 200)))
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

      ui.add_space(4.0);

      time_series_graph(
        ui,
        &mut collision_metrics.generation_time,
        TimeSeriesGraphConfig {
          label: "Collision",
          unit: "ms",
          line_color: egui::Color32::from_rgb(100, 255, 200),
          ..Default::default()
        },
      );

      // Slowest samples widget
      let slowest = profiler_metrics.slowest();
      if !slowest.is_empty() {
        ui.add_space(8.0);

        ui.label(
          egui::RichText::new("Slowest This Frame")
            .color(egui::Color32::from_rgb(180, 180, 180))
            .monospace()
            .size(10.0),
        );

        ui.add_space(2.0);

        for sample in slowest {
          let text = format!("{:>6.2}ms  {}", sample.time_ms, sample.tag);
          ui.label(
            egui::RichText::new(text)
              .color(egui::Color32::from_rgb(255, 255, 180))
              .monospace()
              .size(9.0),
          );
        }
      }
    });
}
