//! Integration tests for polygon triangulation.

use bevy::math::Vec2;
use bevy_pixel_world::collision::{point_in_polygon, triangulate_polygon};

#[test]
fn triangulate_triangle() {
  let triangle = vec![
    Vec2::new(0.0, 0.0),
    Vec2::new(1.0, 0.0),
    Vec2::new(0.5, 1.0),
  ];

  let result = triangulate_polygon(&triangle);
  assert_eq!(result.len(), 1);
}

#[test]
fn triangulate_square() {
  // Counter-clockwise square
  let square = vec![
    Vec2::new(0.0, 0.0),
    Vec2::new(1.0, 0.0),
    Vec2::new(1.0, 1.0),
    Vec2::new(0.0, 1.0),
  ];

  let result = triangulate_polygon(&square);
  assert_eq!(result.len(), 2, "Square should produce 2 triangles");
}

#[test]
fn triangulate_pentagon() {
  // Regular pentagon (counter-clockwise)
  let pentagon = vec![
    Vec2::new(0.0, 1.0),
    Vec2::new(0.951, 0.309),
    Vec2::new(0.588, -0.809),
    Vec2::new(-0.588, -0.809),
    Vec2::new(-0.951, 0.309),
  ];

  let result = triangulate_polygon(&pentagon);
  assert_eq!(result.len(), 3, "Pentagon should produce 3 triangles");
}

#[test]
fn empty_polygon() {
  let result = triangulate_polygon(&[]);
  assert!(result.is_empty());
}

#[test]
fn two_vertices() {
  let result = triangulate_polygon(&[Vec2::ZERO, Vec2::ONE]);
  assert!(result.is_empty());
}

#[test]
fn point_inside_polygon() {
  let square = vec![
    Vec2::new(0.0, 0.0),
    Vec2::new(1.0, 0.0),
    Vec2::new(1.0, 1.0),
    Vec2::new(0.0, 1.0),
  ];

  // Point inside
  assert!(point_in_polygon(Vec2::new(0.5, 0.5), &square));
  // Point outside
  assert!(!point_in_polygon(Vec2::new(2.0, 2.0), &square));
  // Point outside but within bounding box of convex hull
  assert!(!point_in_polygon(Vec2::new(-0.5, 0.5), &square));
}
