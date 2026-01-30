//! IPC client for FastNoise2 NoiseTool via POSIX shared memory.
//!
//! Communicates with NoiseTool using the shared memory segment
//! `/FastNoise2NodeEditor`. Not available on WASM.

#[cfg(not(target_arch = "wasm32"))]
mod native;

#[cfg(not(target_arch = "wasm32"))]
pub use native::NoiseIpc;

/// Stub implementation for WASM (shared memory not available).
#[cfg(target_arch = "wasm32")]
#[derive(Default)]
pub struct NoiseIpc;

#[cfg(target_arch = "wasm32")]
impl NoiseIpc {
  pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
    Ok(Self)
  }

  pub fn poll(&mut self) -> Option<String> {
    None
  }

  pub fn send_import(&mut self, _ent: &str) {}
}
