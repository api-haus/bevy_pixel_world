//! Virtual camera system for priority-based camera control.
//!
//! Multiple systems can spawn `VirtualCamera` entities with different
//! priorities. The highest-priority one controls the real camera. This
//! decouples camera ownership from rendering.
//!
//! # Usage
//!
//! ```ignore
//! use bevy_pixel_world::virtual_camera::{VirtualCameraPlugin, VirtualCamera, ActiveVirtualCamera};
//!
//! app.add_plugins(VirtualCameraPlugin);
//!
//! // Spawn a player-follow camera (default priority)
//! commands.spawn((
//!     VirtualCamera::new(VirtualCamera::PRIORITY_PLAYER),
//!     Transform::default(),
//! ));
//!
//! // Spawn a debug camera (higher priority, takes over)
//! commands.spawn((
//!     VirtualCamera::new(VirtualCamera::PRIORITY_DEBUG),
//!     Transform::from_xyz(100.0, 200.0, 0.0),
//! ));
//! ```
//!
//! # Priority Conventions
//!
//! | Priority | Use Case |
//! |----------|----------|
//! | 0 | Player follow (default) |
//! | 50 | Cutscenes, scripted sequences |
//! | 100 | Debug controller |
//! | 200+ | Console override |

mod components;
mod resources;
mod systems;

use bevy::prelude::*;
pub use components::VirtualCamera;
pub use resources::ActiveVirtualCamera;
pub use systems::{follow_virtual_camera, select_active_virtual_camera};

/// Plugin for priority-based virtual camera control.
///
/// Add this plugin after `PixelCameraPlugin`. Systems will run in `PostUpdate`
/// before pixel camera snapping.
pub struct VirtualCameraPlugin;

impl Plugin for VirtualCameraPlugin {
  fn build(&self, app: &mut App) {
    app.init_resource::<ActiveVirtualCamera>();

    app.add_systems(
      PostUpdate,
      (
        systems::select_active_virtual_camera,
        systems::follow_virtual_camera,
      )
        .chain(),
    );
  }
}
