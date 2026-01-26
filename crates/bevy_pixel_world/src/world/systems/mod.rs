//! PixelWorld ECS systems.
//!
//! Systems are organized into the streaming module for chunk lifecycle
//! and this module for GPU upload.

#[cfg(not(feature = "headless"))]
mod upload;

#[cfg(not(feature = "headless"))]
pub(crate) use upload::upload_dirty_chunks;
