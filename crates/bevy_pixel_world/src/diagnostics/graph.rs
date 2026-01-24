use bevy_egui::egui::{self, Color32, Pos2, Stroke, Vec2};

use super::time_series::TimeSeries;

/// Configuration for rendering a time series graph.
pub struct TimeSeriesGraphConfig<'a> {
  pub label: &'a str,
  pub unit: &'a str,
  pub size: Vec2,
  pub line_color: Color32,
  pub target_line: Option<f32>,
  pub y_padding: f32,
}

impl Default for TimeSeriesGraphConfig<'_> {
  fn default() -> Self {
    Self {
      label: "Value",
      unit: "",
      size: Vec2::new(200.0, 60.0),
      line_color: Color32::from_rgb(100, 200, 100),
      target_line: None,
      y_padding: 0.1,
    }
  }
}

/// Renders a time series graph widget with rolling bars.
pub fn time_series_graph(
  ui: &mut egui::Ui,
  series: &mut TimeSeries,
  config: TimeSeriesGraphConfig,
) {
  let (response, painter) = ui.allocate_painter(config.size, egui::Sense::hover());
  let rect = response.rect;

  // Dark background
  painter.rect_filled(rect, 2.0, Color32::from_rgb(30, 30, 35));

  if series.is_empty() {
    return;
  }

  // Calculate Y range with padding
  let min_val = series.min();
  let max_val = series.max();
  let range = (max_val - min_val).max(0.001); // Avoid division by zero
  let padding = range * config.y_padding;
  let y_min = 0.0_f32.min(min_val - padding); // Always include 0
  let y_max = max_val + padding;
  let y_range = y_max - y_min;

  let samples = series.samples();
  let sample_count = samples.len();

  // Draw bars (1 pixel wide with 1 pixel gap)
  let bar_width = 1.0;
  let bar_spacing = 2.0;
  let max_bars = (rect.width() / bar_spacing) as usize;
  let bars_to_draw = sample_count.min(max_bars);
  let start_idx = sample_count.saturating_sub(bars_to_draw);

  for (i, &value) in samples.iter().skip(start_idx).enumerate() {
    let x = rect.max.x - (bars_to_draw - i) as f32 * bar_spacing;
    let bar_height = ((value - y_min) / y_range) * rect.height();
    let y_top = rect.max.y - bar_height;

    painter.rect_filled(
      egui::Rect::from_min_size(
        Pos2::new(x, y_top.max(rect.min.y)),
        Vec2::new(bar_width, bar_height.min(rect.height())),
      ),
      0.0,
      config.line_color,
    );
  }

  // Draw target line if present
  if let Some(target) = config.target_line {
    if target >= y_min && target <= y_max {
      let y = rect.max.y - ((target - y_min) / y_range) * rect.height();
      painter.line_segment(
        [Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 200, 100, 100)),
      );
    }
  }

  // Draw stats overlay (two lines)
  let current = series.current().unwrap_or(0.0);
  let avg = series.avg();

  // Use more decimal places for very small values
  let precision = if max_val.abs() < 0.1 { 3 } else { 1 };

  // Line 1: Values (smaller font, bright color)
  let values_text = format!(
    "{:.prec$}{}  avg:{:.prec$}  min:{:.prec$}  max:{:.prec$}",
    current,
    config.unit,
    avg,
    min_val,
    max_val,
    prec = precision
  );

  painter.text(
    Pos2::new(rect.min.x + 4.0, rect.min.y + 2.0),
    egui::Align2::LEFT_TOP,
    values_text,
    egui::FontId::monospace(9.0),
    Color32::from_rgb(255, 255, 180), // Bright yellow for values
  );

  // Line 2: Label name (regular font, light gray)
  painter.text(
    Pos2::new(rect.min.x + 4.0, rect.min.y + 13.0),
    egui::Align2::LEFT_TOP,
    config.label,
    egui::FontId::monospace(10.0),
    Color32::from_rgb(180, 180, 180), // Light gray for label
  );
}
