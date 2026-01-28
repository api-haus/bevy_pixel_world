//! WASM NoiseNode implementation using JS bridge to Emscripten module.

use js_sys::Float32Array;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/../sim2d_noise/js/sim2d_noise_bridge.js")]
extern "C" {
  #[wasm_bindgen(js_name = s2d_create)]
  fn s2d_create(encoded: &str) -> u32;

  #[wasm_bindgen(js_name = s2d_gen_2d)]
  fn s2d_gen_2d(
    handle: u32,
    x_off: f32,
    y_off: f32,
    x_cnt: i32,
    y_cnt: i32,
    x_step: f32,
    y_step: f32,
    seed: i32,
  ) -> Float32Array;

  #[wasm_bindgen(js_name = s2d_destroy)]
  fn s2d_destroy(handle: u32);
}

/// WASM noise node using JS bridge to Emscripten FastNoise2.
pub struct NoiseNode {
  handle: u32,
}

impl NoiseNode {
  /// Create a noise node from an encoded node tree string.
  pub fn from_encoded(encoded: &str) -> Option<Self> {
    let handle = s2d_create(encoded);
    if handle == 0 {
      None
    } else {
      Some(Self { handle })
    }
  }

  /// Generate noise values on a uniform 2D grid.
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
    let result = s2d_gen_2d(
      self.handle,
      x_off,
      y_off,
      x_cnt,
      y_cnt,
      x_step,
      y_step,
      seed,
    );
    result.copy_to(output);
  }

  /// Generate a single noise value at the given position.
  pub fn gen_single_2d(&self, x: f32, y: f32, seed: i32) -> f32 {
    let mut output = [0.0f32; 1];
    let result = s2d_gen_2d(self.handle, x, y, 1, 1, 1.0, 1.0, seed);
    result.copy_to(&mut output);
    output[0]
  }
}

impl Drop for NoiseNode {
  fn drop(&mut self) {
    if self.handle != 0 {
      s2d_destroy(self.handle);
    }
  }
}
