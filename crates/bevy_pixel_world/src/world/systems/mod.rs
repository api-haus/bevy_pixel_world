//! PixelWorld ECS systems.
//!
//! Systems are organized into the streaming module for chunk lifecycle
//! and this module for GPU upload.

mod upload;

pub(crate) use upload::upload_dirty_chunks;
