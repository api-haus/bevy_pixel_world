//! Streaming chunk management.
//!
//! This module provides the infrastructure for infinite world streaming:
//! - [`ChunkPool`]: Pre-allocated memory pool for chunks
//! - [`StreamingWindow`]: Manages active chunks around the camera

mod pool;
mod window;

pub use pool::{ChunkPool, PoolHandle};
pub use window::{ActiveChunk, StreamingWindow, WindowDelta};
