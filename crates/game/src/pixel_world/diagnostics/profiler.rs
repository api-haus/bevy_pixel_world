//! Lightweight in-app profiler for tracking named spans.
//!
//! Accumulates samples over 1 second, showing the worst (max) time per tag.

use std::cell::RefCell;
use std::collections::HashMap;

use bevy::prelude::*;
// WASM compat: std::time::Instant panics on wasm32
use web_time::Instant;

/// A single profiler sample.
#[derive(Clone, Copy)]
pub struct ProfilerSample {
  pub tag: &'static str,
  pub time_ms: f32,
}

/// Tracks profiler samples, aggregated by tag with max() over a 1-second
/// window.
#[derive(Resource)]
pub struct ProfilerMetrics {
  /// Displayed samples (updated every second), sorted descending by time.
  display: Vec<ProfilerSample>,
  /// Accumulator: max time per tag since last display update.
  accumulator: HashMap<&'static str, f32>,
  /// Last time the display was updated.
  last_update: Instant,
  /// Update interval in seconds.
  update_interval_secs: f32,
  /// Max entries to display.
  capacity: usize,
}

impl Default for ProfilerMetrics {
  fn default() -> Self {
    Self {
      display: Vec::with_capacity(10),
      accumulator: HashMap::new(),
      last_update: Instant::now(),
      update_interval_secs: 1.0,
      capacity: 10,
    }
  }
}

impl ProfilerMetrics {
  /// Returns the slowest samples, sorted by time descending.
  pub fn slowest(&self) -> &[ProfilerSample] {
    &self.display
  }

  /// Accumulates a sample, keeping max time per tag.
  fn accumulate(&mut self, sample: ProfilerSample) {
    self
      .accumulator
      .entry(sample.tag)
      .and_modify(|max| *max = max.max(sample.time_ms))
      .or_insert(sample.time_ms);
  }

  /// Checks if it's time to refresh the display, and if so, rebuilds it from
  /// the accumulator.
  fn maybe_refresh_display(&mut self) {
    let elapsed = self.last_update.elapsed().as_secs_f32();
    if elapsed < self.update_interval_secs {
      return;
    }

    // Rebuild display from accumulator
    self.display.clear();
    for (&tag, &time_ms) in &self.accumulator {
      self.display.push(ProfilerSample { tag, time_ms });
    }

    // Sort descending by time
    self
      .display
      .sort_by(|a, b| b.time_ms.partial_cmp(&a.time_ms).unwrap());

    // Truncate to capacity
    self.display.truncate(self.capacity);

    // Reset accumulator and timer
    self.accumulator.clear();
    self.last_update = Instant::now();
  }
}

/// RAII guard that records elapsed time on drop.
pub struct ProfileSpan {
  tag: &'static str,
  start: Instant,
}

impl Drop for ProfileSpan {
  fn drop(&mut self) {
    let elapsed_ms = self.start.elapsed().as_secs_f32() * 1000.0;
    FRAME_SAMPLES.with(|samples| {
      samples.borrow_mut().push(ProfilerSample {
        tag: self.tag,
        time_ms: elapsed_ms,
      });
    });
  }
}

/// Creates a profiler span that records elapsed time when dropped.
///
/// # Example
/// ```ignore
/// let _span = profile("my_function");
/// // ... do work ...
/// // Elapsed time recorded when _span goes out of scope
/// ```
pub fn profile(tag: &'static str) -> ProfileSpan {
  ProfileSpan {
    tag,
    start: Instant::now(),
  }
}

// Thread-local storage for samples collected during the frame.
thread_local! {
  static FRAME_SAMPLES: RefCell<Vec<ProfilerSample>> = const { RefCell::new(Vec::new()) };
}

/// System: Aggregates profiler samples into ProfilerMetrics.
///
/// Accumulates samples each frame, updating the display every second.
pub fn aggregate_profiler_samples(mut metrics: ResMut<ProfilerMetrics>) {
  FRAME_SAMPLES.with(|samples| {
    let mut samples = samples.borrow_mut();
    for sample in samples.drain(..) {
      metrics.accumulate(sample);
    }
  });

  metrics.maybe_refresh_display();
}
