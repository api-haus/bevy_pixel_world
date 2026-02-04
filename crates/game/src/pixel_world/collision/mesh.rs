//! Collision mesh types.

use bevy::math::Vec2;

use super::triangulate::Triangle;

/// Collision geometry for a single tile.
///
/// Contains both polyline outlines and triangulated meshes for physics.
#[derive(Clone, Debug, Default)]
pub struct TileCollisionMesh {
  /// Closed polylines representing terrain boundaries.
  /// Points are in world coordinates (f32 for gizmo rendering).
  pub polylines: Vec<Vec<Vec2>>,

  /// Triangulated mesh for physics collision detection.
  /// Each entry contains the polygon vertices and triangle indices.
  pub triangles: Vec<PolygonMesh>,

  /// Generation counter for cache invalidation tracking.
  /// Incremented each time the mesh is regenerated.
  pub generation: u64,

  /// Time spent generating this mesh.
  pub generation_time_ms: f32,
}

/// A triangulated polygon mesh.
#[derive(Clone, Debug)]
pub struct PolygonMesh {
  /// Vertices of the polygon in world coordinates.
  pub vertices: Vec<Vec2>,
  /// Triangle indices into the vertices array.
  pub indices: Vec<Triangle>,
}

impl TileCollisionMesh {
  /// Returns true if this mesh has no geometry.
  pub fn is_empty(&self) -> bool {
    self.polylines.is_empty()
  }

  /// Returns the total number of vertices across all polylines.
  pub fn vertex_count(&self) -> usize {
    self.polylines.iter().map(|p| p.len()).sum()
  }

  /// Returns the total number of triangles across all polygon meshes.
  pub fn triangle_count(&self) -> usize {
    self.triangles.iter().map(|m| m.indices.len()).sum()
  }
}
