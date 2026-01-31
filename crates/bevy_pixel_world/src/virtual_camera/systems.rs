//! Systems for virtual camera selection and following.

use bevy::prelude::*;

use super::components::VirtualCamera;
use super::resources::ActiveVirtualCamera;
use crate::pixel_camera::PixelSceneCamera;

/// System: Selects the active virtual camera based on priority.
///
/// Runs in `PostUpdate` before `follow_virtual_camera`.
///
/// Selection rules:
/// 1. Highest priority wins
/// 2. On tie: prefer currently active (hysteresis)
/// 3. On tie with no current: lowest Entity (deterministic)
pub fn select_active_virtual_camera(
  mut active: ResMut<ActiveVirtualCamera>,
  cameras: Query<(Entity, &VirtualCamera)>,
) {
  let current_active = active.entity;

  // Comparison key: (priority, is_active, inverse_entity)
  // Higher priority wins; on tie, active camera wins; on tie, lower Entity wins
  let best = cameras.iter().max_by_key(|(entity, vc)| {
    let is_active = current_active == Some(*entity);
    // Invert entity bits so lower Entity compares higher
    let inverse_entity = !entity.to_bits();
    (vc.priority, is_active, inverse_entity)
  });

  active.entity = best.map(|(e, _)| e);
}

/// System: Copies the active virtual camera's transform to the real camera.
///
/// Runs in `PostUpdate` after selection, before pixel camera snapping.
pub fn follow_virtual_camera(
  active: Res<ActiveVirtualCamera>,
  virtual_cameras: Query<&Transform, (With<VirtualCamera>, Without<PixelSceneCamera>)>,
  mut real_camera: Query<&mut Transform, With<PixelSceneCamera>>,
) {
  let Some(active_entity) = active.entity else {
    return;
  };

  let Ok(vc_transform) = virtual_cameras.get(active_entity) else {
    return;
  };

  let Ok(mut camera_transform) = real_camera.single_mut() else {
    return;
  };

  // Copy position from virtual camera to real camera
  camera_transform.translation.x = vc_transform.translation.x;
  camera_transform.translation.y = vc_transform.translation.y;
  // Keep the real camera's z (typically 0 or a render layer offset)
}
