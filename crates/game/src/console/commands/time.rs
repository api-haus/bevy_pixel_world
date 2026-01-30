//! Time of day command.

use bevy::prelude::*;
use bevy_console::{ConsoleCommand, reply};
use clap::Parser;

use crate::time_of_day::TimeOfDay;

#[derive(Parser, ConsoleCommand)]
#[command(name = "time")]
pub struct TimeCommand {
  /// Time value (e.g., "12am", "6pm", "14:00", "18")
  value: String,
}

pub fn time_command(mut log: ConsoleCommand<TimeCommand>, mut time: ResMut<TimeOfDay>) {
  if let Some(Ok(TimeCommand { value })) = log.take() {
    match parse_time(&value) {
      Some(hour) => {
        time.hour = hour;
        reply!(log, "Set time to {:.1}", hour);
      }
      None => {
        reply!(log, "Invalid time format. Use: 12am, 6pm, 14:00, or 18");
      }
    }
  }
}

/// Parse time string into hour (0.0-24.0).
/// Supports formats: "12am", "6pm", "14:00", "18", "6:30pm"
fn parse_time(s: &str) -> Option<f32> {
  let s = s.trim().to_lowercase();

  // Try AM/PM format first
  if let Some(stripped) = s.strip_suffix("am") {
    return parse_12h(stripped, false);
  }
  if let Some(stripped) = s.strip_suffix("pm") {
    return parse_12h(stripped, true);
  }

  // Try 24h format with colon (14:00, 14:30)
  if let Some((h, m)) = s.split_once(':') {
    let hour: f32 = h.parse().ok()?;
    let minute: f32 = m.parse().ok()?;
    if hour >= 0.0 && hour < 24.0 && minute >= 0.0 && minute < 60.0 {
      return Some(hour + minute / 60.0);
    }
    return None;
  }

  // Try plain number (18 = 18:00)
  if let Ok(hour) = s.parse::<f32>() {
    if hour >= 0.0 && hour < 24.0 {
      return Some(hour);
    }
  }

  None
}

fn parse_12h(s: &str, is_pm: bool) -> Option<f32> {
  // Handle formats like "6" or "6:30"
  let (hour, minute) = if let Some((h, m)) = s.split_once(':') {
    (h.parse::<f32>().ok()?, m.parse::<f32>().ok()?)
  } else {
    (s.parse::<f32>().ok()?, 0.0)
  };

  if hour < 1.0 || hour > 12.0 || minute < 0.0 || minute >= 60.0 {
    return None;
  }

  let mut result = hour + minute / 60.0;

  // Convert to 24h
  if is_pm && hour != 12.0 {
    result += 12.0;
  } else if !is_pm && hour == 12.0 {
    result = minute / 60.0; // 12am = 0:xx
  }

  Some(result)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_time() {
    assert!((parse_time("12am").unwrap() - 0.0).abs() < 0.01);
    assert!((parse_time("12pm").unwrap() - 12.0).abs() < 0.01);
    assert!((parse_time("6am").unwrap() - 6.0).abs() < 0.01);
    assert!((parse_time("6pm").unwrap() - 18.0).abs() < 0.01);
    assert!((parse_time("14:00").unwrap() - 14.0).abs() < 0.01);
    assert!((parse_time("14:30").unwrap() - 14.5).abs() < 0.01);
    assert!((parse_time("18").unwrap() - 18.0).abs() < 0.01);
    assert!((parse_time("6:30pm").unwrap() - 18.5).abs() < 0.01);
  }
}
