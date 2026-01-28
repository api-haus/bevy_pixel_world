//! Plugin for pixel body lifecycle: spawning, blitting, readback, splitting,
//! collision, and body persistence.
//!
//! Requires [`PixelWorldPlugin`](crate::PixelWorldPlugin) to be added first.

use bevy::ecs::schedule::ApplyDeferred;
use bevy::prelude::*;

use crate::collision::draw_collision_gizmos;
#[cfg(physics)]
use crate::collision::physics::{PhysicsColliderRegistry, sync_physics_colliders};
use crate::collision::{
  CollisionCache, CollisionConfig, CollisionTasks, dispatch_collision_tasks,
  invalidate_dirty_tiles, poll_collision_tasks,
};
use crate::pixel_body::{
  PixelBodyIdGenerator, apply_readback_changes, detect_external_erasure,
  finalize_pending_pixel_bodies, readback_pixel_bodies, split_pixel_bodies,
  sync_simulation_to_bodies, update_pixel_bodies,
};
use crate::schedule::{PixelWorldSet, SimulationPhase};
use crate::world::body_loader::spawn_pending_pixel_bodies;
use crate::world::persistence_systems::{
  save_pixel_bodies_on_chunk_unload, save_pixel_bodies_on_request,
};
use crate::world::streaming::{
  PendingPixelBodies, queue_pixel_bodies_on_chunk_seed, update_simulation_bounds,
};

/// Plugin for pixel body systems: spawning, simulation integration, collision,
/// and body-specific persistence.
///
/// Must be added after [`PixelWorldPlugin`](crate::PixelWorldPlugin). Panics at
/// startup if `PixelWorldPlugin` is missing.
#[derive(Default)]
pub struct PixelBodiesPlugin;

impl Plugin for PixelBodiesPlugin {
  fn build(&self, app: &mut App) {
    // Validate dependency
    assert!(
      app.is_plugin_added::<crate::PixelWorldPlugin>(),
      "PixelBodiesPlugin requires PixelWorldPlugin to be added first"
    );

    app
      .init_resource::<CollisionCache>()
      .init_resource::<CollisionTasks>()
      .init_resource::<CollisionConfig>()
      .init_resource::<PendingPixelBodies>()
      .init_resource::<PixelBodyIdGenerator>()
      .init_resource::<crate::diagnostics::CollisionMetrics>();

    #[cfg(physics)]
    app.init_resource::<PhysicsColliderRegistry>();

    // Pre-simulation body systems, ordered after core streaming systems.
    // Must run after update_simulation_bounds (last in world chain) so that
    // UnloadingChunks and SeededChunks are populated for the current frame.
    app.add_systems(
      Update,
      (
        save_pixel_bodies_on_chunk_unload,
        queue_pixel_bodies_on_chunk_seed,
        finalize_pending_pixel_bodies,
      )
        .chain()
        .after(update_simulation_bounds)
        .in_set(PixelWorldSet::PreSimulation),
    );

    // ApplyDeferred between pre-sim and sim so new bodies are visible
    app.add_systems(
      Update,
      ApplyDeferred
        .after(PixelWorldSet::PreSimulation)
        .before(PixelWorldSet::Simulation),
    );

    // Before CA tick: blit bodies and detect erasure
    app.add_systems(
      Update,
      (detect_external_erasure, update_pixel_bodies)
        .chain()
        .in_set(SimulationPhase::BeforeCATick),
    );

    // After CA tick: readback, shape changes, split, invalidate
    app.add_systems(
      Update,
      (
        sync_simulation_to_bodies,
        readback_pixel_bodies,
        apply_readback_changes,
        split_pixel_bodies,
        invalidate_dirty_tiles,
      )
        .chain()
        .in_set(SimulationPhase::AfterCATick),
    );

    // Post-simulation: collision, spawning, body persistence
    app.add_systems(
      Update,
      (
        dispatch_collision_tasks,
        poll_collision_tasks,
        spawn_pending_pixel_bodies,
        save_pixel_bodies_on_request,
      )
        .chain()
        .in_set(PixelWorldSet::PostSimulation),
    );

    // Physics collider sync (after collision polling)
    #[cfg(physics)]
    app.add_systems(
      Update,
      sync_physics_colliders
        .after(poll_collision_tasks)
        .in_set(PixelWorldSet::PostSimulation),
    );

    // Debug collision gizmos (only when rendering is available)
    app.add_systems(
      PostUpdate,
      draw_collision_gizmos.run_if(resource_exists::<crate::world::plugin::RenderingEnabled>),
    );
  }
}
