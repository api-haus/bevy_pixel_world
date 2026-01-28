//! WASM binary entry point for Emscripten builds.
//!
//! This minimal binary exists to:
//! 1. Provide the `main` function required by Emscripten
//! 2. Link the wasm_api exports from the library
//!
//! The actual C-API implementation lives in `native.rs::wasm_api`.

// Reference the wasm_api module to ensure C-API symbols are linked.
#[cfg(all(target_arch = "wasm32", target_os = "emscripten"))]
use sim2d_noise::wasm_api as _;

/// Required main function for Emscripten.
fn main() {}
