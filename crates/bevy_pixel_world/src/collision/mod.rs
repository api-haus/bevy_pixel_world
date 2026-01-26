//! Collision mesh generation for pixel terrain.
//!
//! This module provides async collision mesh generation using marching squares
//! contour extraction with Douglas-Peucker simplification.
//!
//! # Architecture
//!
//! The collision system works as follows:
//! 1. Entities with `CollisionQueryPoint` trigger mesh generation around their
//!    position
//! 2. Nearby tiles are checked for cached meshes; missing ones spawn async
//!    tasks
//! 3. Tasks extract pixel data, run marching squares, and simplify the result
//! 4. Completed meshes are cached and rendered as gizmos for debugging
//!
//! # Usage
//!
//! ```ignore
//! // Add CollisionQueryPoint to an entity to generate meshes around it
//! commands.spawn((
//!     Transform::default(),
//!     CollisionQueryPoint,
//! ));
//!
//! // Configure collision generation
//! app.insert_resource(CollisionConfig {
//!     simplification_tolerance: 1.0,
//!     proximity_radius: 3,
//!     debug_gizmos: true,
//! });
//! ```

mod cache;
mod contour;
#[cfg(feature = "visual_debug")]
mod debug;
mod marching;
mod mesh;
mod simplify;
mod systems;
mod triangulate;

#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
pub mod physics;

use bevy::prelude::*;
pub use cache::{CollisionCache, CollisionTask, CollisionTasks};
pub use contour::{EDGE_TABLE, connect_segments, extract_marching_segments, grid_key};
#[cfg(feature = "visual_debug")]
pub use debug::{SampleMesh, draw_sample_mesh_gizmos, update_sample_mesh};
pub use marching::{GRID_SIZE, marching_squares};
pub use mesh::{PolygonMesh, TileCollisionMesh};
pub use simplify::{douglas_peucker, simplify_polylines};
#[cfg(feature = "visual_debug")]
pub use systems::draw_collision_gizmos;
pub use systems::{
  CollisionQueryPoint, dispatch_collision_tasks, invalidate_dirty_tiles, poll_collision_tasks,
};
pub use triangulate::{Triangle, point_in_polygon, triangulate_polygon, triangulate_polygons};

use crate::coords::TilePos;

/// Marker for loaded pixel bodies waiting for terrain collision.
///
/// Bodies with this component remain static until all required
/// collision tiles are cached, then upgrade to dynamic.
#[derive(Component)]
pub struct AwaitingCollision {
  /// Tiles that must be in CollisionCache before enabling dynamics.
  pub required_tiles: Vec<TilePos>,
}

/// Marker for bodies in stabilization period after activation.
///
/// Bodies with this component skip external erasure and readback detection,
/// giving physics time to separate overlapping bodies before checking for
/// pixel destruction.
#[derive(Component)]
pub struct Stabilizing {
  /// Frames remaining in stabilization period.
  pub frames_remaining: u32,
}

impl Default for Stabilizing {
  fn default() -> Self {
    Self {
      frames_remaining: 10, // ~0.17 sec at 60fps
    }
  }
}

/// Configuration for collision mesh generation.
#[derive(Resource, Clone, Debug)]
pub struct CollisionConfig {
  /// Douglas-Peucker simplification tolerance in pixels.
  /// Higher values produce simpler meshes with fewer vertices.
  /// Default: 1.0
  pub simplification_tolerance: f32,

  /// Radius in tiles around query points to generate meshes.
  /// A radius of 3 means a 7x7 tile area (49 tiles).
  /// Default: 3
  pub proximity_radius: u32,

  /// Whether to render collision meshes as debug gizmos.
  /// Default: true
  pub debug_gizmos: bool,
}

impl Default for CollisionConfig {
  fn default() -> Self {
    Self {
      simplification_tolerance: 1.0,
      proximity_radius: 3,
      debug_gizmos: true,
    }
  }
}

impl CollisionConfig {
  /// Creates a new config with the given simplification tolerance.
  pub fn with_tolerance(mut self, tolerance: f32) -> Self {
    self.simplification_tolerance = tolerance;
    self
  }

  /// Creates a new config with the given proximity radius.
  pub fn with_radius(mut self, radius: u32) -> Self {
    self.proximity_radius = radius;
    self
  }

  /// Enables or disables debug gizmo rendering.
  pub fn with_gizmos(mut self, enabled: bool) -> Self {
    self.debug_gizmos = enabled;
    self
  }
}
