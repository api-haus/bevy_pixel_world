//! Debug utilities for collision mesh visualization.
//!
//! This module provides sample mesh generation and gizmo rendering
//! for testing and debugging collision mesh systems.

use bevy::math::Vec2;
use bevy::prelude::*;

use super::mesh::PolygonMesh;
use super::systems::CollisionQueryPoint;
use super::triangulate::triangulate_polygon;

/// Shape types for sample mesh.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum SampleShapeType {
  #[default]
  Hexagon,
  Star,
  LShape,
}

/// Resource holding a sample mesh for visualization testing.
#[derive(Resource, Default)]
pub struct SampleMesh {
  /// The polygon mesh to visualize.
  pub mesh: Option<PolygonMesh>,
  /// Position in world coordinates.
  pub position: Vec2,
  /// Whether to show the sample mesh.
  pub enabled: bool,
  /// Current shape type (used to avoid regenerating).
  pub shape_type: SampleShapeType,
}

impl SampleMesh {
  /// Creates a regular polygon (circle approximation) centered at position.
  pub fn regular_polygon(center: Vec2, radius: f32, num_vertices: usize) -> PolygonMesh {
    use std::f32::consts::PI;
    let vertices: Vec<Vec2> = (0..num_vertices)
      .map(|i| {
        let angle = 2.0 * PI * (i as f32) / (num_vertices as f32) - PI / 2.0;
        Vec2::new(
          center.x + radius * angle.cos(),
          center.y + radius * angle.sin(),
        )
      })
      .collect();

    let indices = triangulate_polygon(&vertices);

    PolygonMesh { vertices, indices }
  }

  /// Creates a star shape centered at position.
  pub fn star(
    center: Vec2,
    outer_radius: f32,
    inner_radius: f32,
    num_points: usize,
  ) -> PolygonMesh {
    use std::f32::consts::PI;
    let mut vertices = Vec::with_capacity(num_points * 2);

    for i in 0..(num_points * 2) {
      let angle = PI * (i as f32) / (num_points as f32) - PI / 2.0;
      let radius = if i % 2 == 0 {
        outer_radius
      } else {
        inner_radius
      };
      vertices.push(Vec2::new(
        center.x + radius * angle.cos(),
        center.y + radius * angle.sin(),
      ));
    }

    let indices = triangulate_polygon(&vertices);

    PolygonMesh { vertices, indices }
  }

  /// Creates an L-shaped polygon (concave) centered at position.
  pub fn l_shape(center: Vec2, size: f32) -> PolygonMesh {
    let half = size / 2.0;
    let third = size / 3.0;

    // L-shape vertices (counter-clockwise)
    let vertices = vec![
      Vec2::new(center.x - half, center.y - half), // bottom-left
      Vec2::new(center.x - half + third, center.y - half), // bottom inner
      Vec2::new(center.x - half + third, center.y + third), // inner corner
      Vec2::new(center.x + half, center.y + third), // right inner
      Vec2::new(center.x + half, center.y + half), // top-right
      Vec2::new(center.x - half, center.y + half), // top-left
    ];

    let indices = triangulate_polygon(&vertices);

    PolygonMesh { vertices, indices }
  }
}

/// Returns the shape type selected by key press, if any.
fn shape_key_pressed(keys: &ButtonInput<KeyCode>) -> Option<SampleShapeType> {
  if keys.just_pressed(KeyCode::Digit1) {
    bevy::log::info!("Sample mesh: Hexagon");
    Some(SampleShapeType::Hexagon)
  } else if keys.just_pressed(KeyCode::Digit2) {
    bevy::log::info!("Sample mesh: Star");
    Some(SampleShapeType::Star)
  } else if keys.just_pressed(KeyCode::Digit3) {
    bevy::log::info!("Sample mesh: L-shape (concave)");
    Some(SampleShapeType::LShape)
  } else {
    None
  }
}

/// System: Updates the sample mesh position to follow the cursor.
pub fn update_sample_mesh(
  mut sample_mesh: ResMut<SampleMesh>,
  query_points: Query<&Transform, With<CollisionQueryPoint>>,
  keys: Res<ButtonInput<KeyCode>>,
) {
  // Toggle sample mesh with 'T' key
  if keys.just_pressed(KeyCode::KeyT) {
    sample_mesh.enabled = !sample_mesh.enabled;
    bevy::log::info!(
      "Sample mesh {}",
      if sample_mesh.enabled {
        "enabled - press 1/2/3 to switch shapes"
      } else {
        "disabled"
      }
    );
  }

  if !sample_mesh.enabled {
    sample_mesh.mesh = None;
    return;
  }

  // Get cursor position from collision query point
  let Ok(transform) = query_points.single() else {
    return;
  };
  let cursor_pos = transform.translation.truncate();

  // Check for shape change
  let new_shape = shape_key_pressed(&keys);
  if let Some(shape) = new_shape {
    sample_mesh.shape_type = shape;
  }

  // Generate mesh if none exists or shape changed
  if new_shape.is_some() || sample_mesh.mesh.is_none() {
    let radius = 50.0;
    sample_mesh.mesh = Some(match sample_mesh.shape_type {
      SampleShapeType::Hexagon => SampleMesh::regular_polygon(cursor_pos, radius, 6),
      SampleShapeType::Star => SampleMesh::star(cursor_pos, radius, radius * 0.4, 5),
      SampleShapeType::LShape => SampleMesh::l_shape(cursor_pos, radius * 2.0),
    });
    sample_mesh.position = cursor_pos;
  } else {
    // Translate existing vertices to follow cursor (no regeneration)
    let delta = cursor_pos - sample_mesh.position;
    if delta != Vec2::ZERO {
      if let Some(mesh) = &mut sample_mesh.mesh {
        for v in &mut mesh.vertices {
          *v += delta;
        }
      }
      sample_mesh.position = cursor_pos;
    }
  }
}

/// System: Draws the sample mesh as debug gizmos.
pub fn draw_sample_mesh_gizmos(sample_mesh: Res<SampleMesh>, mut gizmos: Gizmos) {
  let Some(mesh) = &sample_mesh.mesh else {
    return;
  };

  if !sample_mesh.enabled {
    return;
  }

  // Green color for collision mesh edges
  let edge_color = Color::srgb(0.2, 0.8, 0.3);

  // Draw triangle edges only
  for triangle in &mesh.indices {
    let a = mesh.vertices[triangle.a];
    let b = mesh.vertices[triangle.b];
    let c = mesh.vertices[triangle.c];

    gizmos.line_2d(a, b, edge_color);
    gizmos.line_2d(b, c, edge_color);
    gizmos.line_2d(c, a, edge_color);
  }
}
