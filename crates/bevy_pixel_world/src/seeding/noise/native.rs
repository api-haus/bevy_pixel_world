//! Native NoiseNode implementation using fastnoise2 directly.

use fastnoise2::SafeNode;

/// Native noise node wrapping fastnoise2 SafeNode.
pub struct NoiseNode {
  inner: SafeNode,
}

impl NoiseNode {
  /// Create a noise node from an encoded node tree string.
  pub fn from_encoded(encoded: &str) -> Option<Self> {
    SafeNode::from_encoded_node_tree(encoded)
      .ok()
      .map(|inner| Self { inner })
  }

  /// Generate noise values on a uniform 2D grid.
  #[allow(clippy::too_many_arguments)]
  pub fn gen_uniform_grid_2d(
    &self,
    output: &mut [f32],
    x_off: f32,
    y_off: f32,
    x_cnt: i32,
    y_cnt: i32,
    x_step: f32,
    y_step: f32,
    seed: i32,
  ) {
    self
      .inner
      .gen_uniform_grid_2d(output, x_off, y_off, x_cnt, y_cnt, x_step, y_step, seed);
  }
}

// NoiseNode is Send + Sync because SafeNode is
unsafe impl Send for NoiseNode {}
unsafe impl Sync for NoiseNode {}
