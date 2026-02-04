//! Resources for the virtual camera system.

use bevy::prelude::*;

/// Tracks which virtual camera is currently active.
///
/// Updated by `select_active_virtual_camera` each frame.
/// Systems can read this to know which camera controls the view.
#[derive(Resource, Default)]
pub struct ActiveVirtualCamera {
  /// The entity of the currently active virtual camera, if any.
  pub entity: Option<Entity>,
}
