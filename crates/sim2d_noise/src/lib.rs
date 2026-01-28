//! FastNoise2 noise generation with native and WASM support.
//!
//! This crate provides a Rust wrapper around FastNoise2 C++ library via FFI.
//! Both native and WASM (Emscripten) builds use the same core implementation.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     native.rs                               │
//! │  ┌───────────────────────────────────────────────────────┐  │
//! │  │ NoiseNode (Rust API)                                  │  │
//! │  │   - from_encoded()                                    │  │
//! │  │   - gen_uniform_grid_2d()                             │  │
//! │  └───────────────────────────────────────────────────────┘  │
//! │  ┌───────────────────────────────────────────────────────┐  │
//! │  │ wasm_api (C-ABI exports, wasm32 only)                 │  │
//! │  │   - s2d_noise_create()                                │  │
//! │  │   - s2d_noise_gen_2d()                                │  │
//! │  │   - s2d_noise_destroy()                               │  │
//! │  └───────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # WASM Emscripten Build
//! ```bash
//! cd crates/sim2d_noise && make build
//! ```
//! Produces `dist/sim2d_noise.js` + `dist/sim2d_noise.wasm`.
//! The JS bridge (`js/sim2d_noise_bridge.js`) wraps these exports.

mod native;
pub use native::NoiseNode;

// Re-export wasm_api for Emscripten builds
#[cfg(all(target_arch = "wasm32", target_os = "emscripten"))]
pub use native::wasm_api;

/// Encoded node tree presets (from FastNoise2 NoiseTool)
pub mod presets {
  /// Simplex noise for terrain generation
  pub const SIMPLEX: &str = "BwAAgEVDCBY@BE";
}

#[cfg(test)]
mod tests {
  use super::{presets, NoiseNode};

  #[test]
  fn test_simplex() {
    let node = NoiseNode::from_encoded(presets::SIMPLEX).expect("Failed to create noise node");
    let mut output = vec![0.0f32; 32 * 32];
    node.gen_uniform_grid_2d(&mut output, 0.0, 0.0, 32, 32, 1.0, 1.0, 1337);
    assert!(output.iter().any(|&v| v != 0.0), "All values are zero");
  }
}
