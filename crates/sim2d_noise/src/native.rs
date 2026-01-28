//! Noise generation - Native FastNoise2 wrapper with WASM C-API exports.
//!
//! This module provides:
//! - `NoiseNode`: Rust API wrapping FastNoise2's SafeNode
//! - `wasm_api`: C-ABI exports for Emscripten JS bridge (wasm32 only)
//!
//! Both pathways use the same underlying NoiseNode implementation.

use fastnoise2::SafeNode;

// ============================================================================
// NoiseNode - Rust API (all targets)
// ============================================================================

/// A noise generator node created from an encoded node tree string.
///
/// Encoded strings can be exported from FastNoise2's NoiseTool application.
/// This provides a simple way to design complex noise graphs visually and
/// use them in code.
pub struct NoiseNode {
  inner: SafeNode,
}

impl NoiseNode {
  /// Create a noise node from an encoded node tree string.
  ///
  /// Returns `None` if the encoded string is invalid.
  pub fn from_encoded(encoded: &str) -> Option<Self> {
    SafeNode::from_encoded_node_tree(encoded)
      .ok()
      .map(|inner| Self { inner })
  }

  /// Generate noise values on a uniform 2D grid.
  ///
  /// # Arguments
  /// * `output` - Buffer to write noise values into (must be x_cnt * y_cnt in size)
  /// * `x_off, y_off` - Grid origin offset
  /// * `x_cnt, y_cnt` - Grid dimensions
  /// * `x_step, y_step` - Step size between samples
  /// * `seed` - Random seed
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

// ============================================================================
// WASM C-API Exports (wasm32-emscripten only)
// ============================================================================
//
// These functions are exported for the Emscripten JS bridge.
// The JS bridge (`sim2d_noise_bridge.js`) calls these via the Emscripten module.
//
// Build: `make build` in crates/sim2d_noise/
// Output: dist/sim2d_noise.js + dist/sim2d_noise.wasm

#[cfg(all(target_arch = "wasm32", target_os = "emscripten"))]
pub mod wasm_api {
  use super::NoiseNode;
  use std::ffi::CStr;
  use std::os::raw::c_char;

  /// Create a noise node from an encoded node tree string.
  ///
  /// Returns a handle (pointer as usize) or 0 on failure.
  #[no_mangle]
  pub extern "C" fn s2d_noise_create(encoded: *const c_char) -> usize {
    if encoded.is_null() {
      return 0;
    }

    let encoded_str = match unsafe { CStr::from_ptr(encoded) }.to_str() {
      Ok(s) => s,
      Err(_) => return 0,
    };

    match NoiseNode::from_encoded(encoded_str) {
      Some(node) => Box::into_raw(Box::new(node)) as usize,
      None => 0,
    }
  }

  /// Generate noise values on a uniform 2D grid.
  ///
  /// # Safety
  /// - `handle` must be a valid pointer from `s2d_noise_create`
  /// - `output` must point to a buffer of at least `x_cnt * y_cnt` f32s
  #[no_mangle]
  pub extern "C" fn s2d_noise_gen_2d(
    handle: usize,
    output: *mut f32,
    x_off: f32,
    y_off: f32,
    x_cnt: i32,
    y_cnt: i32,
    x_step: f32,
    y_step: f32,
    seed: i32,
  ) {
    if handle == 0 || output.is_null() {
      return;
    }

    let node = unsafe { &*(handle as *const NoiseNode) };
    let count = (x_cnt * y_cnt) as usize;
    let output_slice = unsafe { std::slice::from_raw_parts_mut(output, count) };

    node.gen_uniform_grid_2d(output_slice, x_off, y_off, x_cnt, y_cnt, x_step, y_step, seed);
  }

  /// Destroy a noise node and free its memory.
  ///
  /// # Safety
  /// - `handle` must be a valid pointer from `s2d_noise_create`, or 0 (no-op)
  /// - Must not be called twice with the same handle
  #[no_mangle]
  pub extern "C" fn s2d_noise_destroy(handle: usize) {
    if handle == 0 {
      return;
    }

    unsafe {
      drop(Box::from_raw(handle as *mut NoiseNode));
    }
  }
}
