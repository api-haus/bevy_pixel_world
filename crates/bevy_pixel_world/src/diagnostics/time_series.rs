use std::collections::VecDeque;

/// A ring buffer for storing time-series samples with cached statistics.
pub struct TimeSeries {
  samples: VecDeque<f32>,
  capacity: usize,
  min_cached: f32,
  max_cached: f32,
  sum_cached: f32,
  dirty: bool, // min/max need recalculation
}

impl TimeSeries {
  pub fn new(capacity: usize) -> Self {
    Self {
      samples: VecDeque::with_capacity(capacity),
      capacity,
      min_cached: f32::MAX,
      max_cached: f32::MIN,
      sum_cached: 0.0,
      dirty: false,
    }
  }

  pub fn push(&mut self, value: f32) {
    if self.samples.len() >= self.capacity {
      if let Some(removed) = self.samples.pop_front() {
        self.sum_cached -= removed;
        // Mark dirty if the removed value was the min or max
        if removed <= self.min_cached || removed >= self.max_cached {
          self.dirty = true;
        }
      }
    }

    self.samples.push_back(value);
    self.sum_cached += value;

    // Update min/max if the new value extends the range
    if !self.dirty {
      if value < self.min_cached {
        self.min_cached = value;
      }
      if value > self.max_cached {
        self.max_cached = value;
      }
    }
  }

  pub fn samples(&self) -> &VecDeque<f32> {
    &self.samples
  }

  pub fn is_empty(&self) -> bool {
    self.samples.is_empty()
  }

  pub fn current(&self) -> Option<f32> {
    self.samples.back().copied()
  }

  pub fn min(&mut self) -> f32 {
    self.recalculate_if_dirty();
    self.min_cached
  }

  pub fn max(&mut self) -> f32 {
    self.recalculate_if_dirty();
    self.max_cached
  }

  pub fn avg(&self) -> f32 {
    if self.samples.is_empty() {
      0.0
    } else {
      self.sum_cached / self.samples.len() as f32
    }
  }

  fn recalculate_if_dirty(&mut self) {
    if self.dirty {
      self.min_cached = f32::MAX;
      self.max_cached = f32::MIN;
      for &sample in &self.samples {
        if sample < self.min_cached {
          self.min_cached = sample;
        }
        if sample > self.max_cached {
          self.max_cached = sample;
        }
      }
      self.dirty = false;
    }
  }
}
