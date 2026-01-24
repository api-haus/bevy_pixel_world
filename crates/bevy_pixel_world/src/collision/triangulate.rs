//! Polygon triangulation using Constrained Delaunay Triangulation (CDT).
//!
//! Converts closed polylines into triangle meshes suitable for collision
//! detection. Uses spade's CDT which respects polygon edges as constraints.

use bevy::math::Vec2;
use spade::{ConstrainedDelaunayTriangulation, Point2, Triangulation, handles::FixedVertexHandle};

/// A triangle represented by three vertex indices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Triangle {
  pub a: usize,
  pub b: usize,
  pub c: usize,
}

type CDT = ConstrainedDelaunayTriangulation<Point2<f64>>;

/// Validates that a polygon can be triangulated.
fn validate_polygon(polygon: &[Vec2]) -> bool {
  polygon.len() >= 3 && !has_self_intersections(polygon)
}

/// Builds a CDT with polygon vertices and edge constraints.
fn build_constrained_cdt(polygon: &[Vec2]) -> Option<(CDT, Vec<FixedVertexHandle>)> {
  let mut cdt = CDT::new();

  // Insert vertices and collect handles
  let handles: Vec<_> = polygon
    .iter()
    .map(|v| {
      cdt
        .insert(Point2::new(v.x as f64, v.y as f64))
        .expect("Failed to insert vertex into CDT")
    })
    .collect();

  // Add edge constraints (polygon boundary)
  for i in 0..handles.len() {
    let j = (i + 1) % handles.len();
    // add_constraint may fail if vertices are identical; ignore such cases
    let _ = cdt.add_constraint(handles[i], handles[j]);
  }

  Some((cdt, handles))
}

/// Extracts interior triangles from a CDT.
fn extract_interior_triangles(
  cdt: &CDT,
  handles: &[FixedVertexHandle],
  polygon: &[Vec2],
) -> Vec<Triangle> {
  // Build a map from fixed vertex handle to polygon index
  let handle_to_index: std::collections::HashMap<_, _> = handles
    .iter()
    .enumerate()
    .map(|(idx, &handle)| (handle, idx))
    .collect();

  let mut triangles = Vec::new();

  for face in cdt.inner_faces() {
    let verts = face.vertices();

    // Get vertex positions
    let positions: [Vec2; 3] = std::array::from_fn(|i| {
      let pos = verts[i].position();
      Vec2::new(pos.x as f32, pos.y as f32)
    });

    // Calculate triangle centroid
    let centroid = (positions[0] + positions[1] + positions[2]) / 3.0;

    // Only include triangles whose centroid is inside the polygon
    if point_in_polygon(centroid, polygon) {
      // Map handles back to polygon indices
      let idx0 = handle_to_index.get(&verts[0].fix());
      let idx1 = handle_to_index.get(&verts[1].fix());
      let idx2 = handle_to_index.get(&verts[2].fix());

      if let (Some(&i0), Some(&i1), Some(&i2)) = (idx0, idx1, idx2) {
        triangles.push(Triangle {
          a: i0,
          b: i1,
          c: i2,
        });
      }
    }
  }

  triangles
}

/// Triangulates a simple polygon using Constrained Delaunay Triangulation.
///
/// # Arguments
/// * `polygon` - A closed polygon as a list of vertices (counter-clockwise
///   winding).
///
/// # Returns
/// A list of triangles, each represented by three vertex indices into the
/// original polygon. Returns empty if the polygon is invalid
/// (self-intersecting, too few vertices, etc.)
pub fn triangulate_polygon(polygon: &[Vec2]) -> Vec<Triangle> {
  if polygon.len() < 3 {
    return vec![];
  }
  if polygon.len() == 3 {
    return vec![Triangle { a: 0, b: 1, c: 2 }];
  }

  if !validate_polygon(polygon) {
    return vec![];
  }

  let Some((cdt, handles)) = build_constrained_cdt(polygon) else {
    return vec![];
  };

  extract_interior_triangles(&cdt, &handles, polygon)
}

/// Checks if a polygon has any self-intersecting edges.
fn has_self_intersections(polygon: &[Vec2]) -> bool {
  let n = polygon.len();
  for i in 0..n {
    let a1 = polygon[i];
    let a2 = polygon[(i + 1) % n];

    // Check against all non-adjacent edges
    for j in (i + 2)..n {
      // Skip the edge that shares a vertex with edge i
      if j == (i + n - 1) % n {
        continue;
      }

      let b1 = polygon[j];
      let b2 = polygon[(j + 1) % n];

      if segments_intersect(a1, a2, b1, b2) {
        return true;
      }
    }
  }
  false
}

/// Checks if two line segments intersect (excluding shared endpoints).
fn segments_intersect(a1: Vec2, a2: Vec2, b1: Vec2, b2: Vec2) -> bool {
  let d1 = cross_2d(b2 - b1, a1 - b1);
  let d2 = cross_2d(b2 - b1, a2 - b1);
  let d3 = cross_2d(a2 - a1, b1 - a1);
  let d4 = cross_2d(a2 - a1, b2 - a1);

  // Check if segments straddle each other
  if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
    && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
  {
    return true;
  }

  false
}

/// 2D cross product (z-component of 3D cross product).
fn cross_2d(a: Vec2, b: Vec2) -> f32 {
  a.x * b.y - a.y * b.x
}

/// Tests if a point is inside a polygon using the ray casting algorithm.
pub fn point_in_polygon(point: Vec2, polygon: &[Vec2]) -> bool {
  let mut inside = false;
  let n = polygon.len();

  let mut j = n - 1;
  for i in 0..n {
    let vi = polygon[i];
    let vj = polygon[j];

    // Check if the ray from point going right crosses this edge
    if ((vi.y > point.y) != (vj.y > point.y))
      && (point.x < (vj.x - vi.x) * (point.y - vi.y) / (vj.y - vi.y) + vi.x)
    {
      inside = !inside;
    }
    j = i;
  }

  inside
}

/// Triangulates multiple polygons.
pub fn triangulate_polygons(polygons: &[Vec<Vec2>]) -> Vec<(Vec<Vec2>, Vec<Triangle>)> {
  polygons
    .iter()
    .filter(|p| p.len() >= 3)
    .map(|polygon| {
      let triangles = triangulate_polygon(polygon);
      (polygon.clone(), triangles)
    })
    .collect()
}
