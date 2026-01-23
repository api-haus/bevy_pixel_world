//! Debug gizmo rendering systems.

use bevy::prelude::*;

use super::gizmos::{ActiveGizmo, ActiveGizmos, PendingDebugGizmos};

/// System that renders debug gizmos.
///
/// 1. Drains pending gizmos into active gizmos with current timestamp
/// 2. Draws active gizmos as rect outlines
/// 3. Removes expired gizmos
pub fn render_debug_gizmos(
    mut gizmos: Gizmos,
    time: Res<Time>,
    pending: Res<PendingDebugGizmos>,
    mut active: ResMut<ActiveGizmos>,
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
