//! Debug gizmo rendering systems.

use bevy::prelude::*;

use super::gizmos::{ActiveGizmo, ActiveGizmos, PendingDebugGizmos};
use super::settings::VisualDebugSettings;
use crate::pixel_body::PixelBody;
use crate::world::PixelWorld;
use crate::world::control::PersistenceControl;
use crate::world::slot::ChunkLifecycle;

/// System that renders debug gizmos.
///
/// 1. Drains pending gizmos into active gizmos with current timestamp
/// 2. Draws active gizmos as rect outlines (filtered by settings)
/// 3. Removes expired gizmos
pub fn render_debug_gizmos(
  mut gizmos: Gizmos,
  time: Res<Time>,
  pending: Res<PendingDebugGizmos>,
  mut active: ResMut<ActiveGizmos>,
  settings: Option<Res<VisualDebugSettings>>,
) {
  let current_time = time.elapsed_secs();

  // Drain pending into active
  for pending_gizmo in pending.drain() {
    active.gizmos.push(ActiveGizmo {
      kind: pending_gizmo.kind,
      rect: pending_gizmo.rect,
      spawn_time: current_time,
    });
  }

  // Draw active gizmos and collect indices of expired ones
  let mut expired_indices = Vec::new();

  for (i, gizmo) in active.gizmos.iter().enumerate() {
    let age = current_time - gizmo.spawn_time;
    let duration = gizmo.kind.duration();

    if age > duration {
      expired_indices.push(i);
      continue;
    }

    // Skip if this gizmo kind is disabled in settings
    if let Some(ref settings) = settings
      && !settings.is_enabled(gizmo.kind)
    {
      continue;
    }

    // Calculate alpha fade (full opacity for first half, fade out in second half)
    let alpha = if age < duration * 0.5 {
      1.0
    } else {
      1.0 - (age - duration * 0.5) / (duration * 0.5)
    };

    let base_color = gizmo.kind.color();
    let color = base_color.with_alpha(alpha);

    // Calculate rect center and size
    let center_x = gizmo.rect.x as f32 + gizmo.rect.width as f32 / 2.0;
    let center_y = gizmo.rect.y as f32 + gizmo.rect.height as f32 / 2.0;
    let size = Vec2::new(gizmo.rect.width as f32, gizmo.rect.height as f32);

    gizmos.rect_2d(
      Isometry2d::from_translation(Vec2::new(center_x, center_y)),
      size,
      color,
    );
  }

  // Remove expired gizmos (in reverse order to preserve indices)
  for i in expired_indices.into_iter().rev() {
    active.gizmos.swap_remove(i);
  }
}

/// Draws small red circles at the centers of pixel body entities.
pub fn draw_pixel_body_centers(
  mut gizmos: Gizmos,
  settings: Option<Res<VisualDebugSettings>>,
  bodies: Query<&Transform, With<PixelBody>>,
) {
  let Some(settings) = settings else { return };
  if !settings.show_pixel_body_centers {
    return;
  }

  let color = Color::srgb(1.0, 0.2, 0.2);
  let radius = 3.0;

  for transform in bodies.iter() {
    let center = transform.translation.truncate();
    gizmos.circle_2d(center, radius, color);
  }
}

/// Syncs CollisionConfig::debug_gizmos with
/// VisualDebugSettings::show_collision_meshes.
pub fn sync_collision_config(
  settings: Res<VisualDebugSettings>,
  config: Option<ResMut<crate::collision::CollisionConfig>>,
) {
  let Some(mut config) = config else { return };
  if config.debug_gizmos != settings.show_collision_meshes {
    config.debug_gizmos = settings.show_collision_meshes;
  }
}

/// Debug keyboard controls for persistence testing.
///
/// - P: Persist all modified chunks to storage
/// - R: Reload all chunks from storage (re-seed from persisted data)
pub fn debug_persistence_keyboard(
  keyboard: Res<ButtonInput<KeyCode>>,
  mut persistence: Option<ResMut<PersistenceControl>>,
  mut worlds: Query<&mut PixelWorld>,
) {
  // P = Persist all modified chunks
  if keyboard.just_pressed(KeyCode::KeyP) {
    if let Some(ref mut persistence) = persistence {
      if persistence.is_active() {
        persistence.save();
        debug!("[Debug] Triggered persistence for all modified chunks");
      } else {
        warn!("[Debug] No save loaded - persistence not active");
      }
    } else {
      warn!("[Debug] No PersistenceControl available - persistence disabled");
    }
  }

  // R = Reload all chunks from storage
  if keyboard.just_pressed(KeyCode::KeyR) {
    let mut total_reloaded = 0;
    for mut world in worlds.iter_mut() {
      // Collect active chunk positions and indices
      let active: Vec<_> = world.active_chunks().collect();

      for (_pos, idx) in active {
        let slot = world.slot_mut(idx);
        // Only reload chunks that are fully active
        if slot.lifecycle == ChunkLifecycle::Active {
          // Reset to Loading state so dispatch_chunk_loads sends LoadChunk command.
          // This is required on WASM where I/O goes through the worker.
          slot.lifecycle = ChunkLifecycle::Loading;
          slot.dirty = true; // Force GPU re-upload after reload
          slot.modified = false;
          slot.persisted = false;
          total_reloaded += 1;
        }
      }
    }
    if total_reloaded > 0 {
      debug!(
        "[Debug] Queued {} chunks for reload from storage",
        total_reloaded
      );
    } else {
      warn!("[Debug] No active chunks to reload");
    }
  }
}
