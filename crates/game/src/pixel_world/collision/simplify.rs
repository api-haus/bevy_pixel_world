//! Douglas-Peucker polyline simplification.
//!
//! Reduces the number of vertices in a polyline while preserving its shape
//! within a specified tolerance.

use bevy::math::Vec2;

/// Simplifies a polyline using the Douglas-Peucker algorithm.
///
/// # Arguments
/// * `polyline` - A closed polyline (vertices form a loop).
/// * `tolerance` - Maximum allowed perpendicular distance from the simplified
///   line.
///
/// # Returns
/// A simplified polyline with fewer vertices.
pub fn douglas_peucker(polyline: &[Vec2], tolerance: f32) -> Vec<Vec2> {
  if polyline.len() <= 3 {
    return polyline.to_vec();
  }

  // For closed polylines, we need to find the best split point
  // to avoid artifacts at the arbitrary start/end point.
  // We find the two points furthest apart and use those as anchors.
  let (i1, i2) = find_furthest_pair(polyline);

  // Split the polyline at these two points and simplify each half
  let (half1, half2) = split_at_indices(polyline, i1, i2);

  let mut simplified1 = simplify_open(&half1, tolerance);
  let mut simplified2 = simplify_open(&half2, tolerance);

  // half1 ends with the same point that half2 starts with (the second split
  // point) half2 ends with the same point that half1 starts with (the first
  // split point) We need to remove these duplicates to get a clean closed
  // polygon.

  // Remove the last point of half1 (duplicate of half2[0])
  if !simplified1.is_empty() {
    simplified1.pop();
  }

  // Remove the last point of half2 (duplicate of half1[0])
  if !simplified2.is_empty() {
    simplified2.pop();
  }

  // Merge the two halves - half2 still starts with the junction point we just
  // removed from half1, so we don't skip it
  simplified1.extend(simplified2);

  simplified1
}

/// Simplifies multiple polylines.
pub fn simplify_polylines(polylines: Vec<Vec<Vec2>>, tolerance: f32) -> Vec<Vec<Vec2>> {
  polylines
    .into_iter()
    .map(|p| douglas_peucker(&p, tolerance))
    .filter(|p| p.len() >= 3)
    .collect()
}

/// Finds the indices of the two furthest-apart points in a polyline.
fn find_furthest_pair(polyline: &[Vec2]) -> (usize, usize) {
  let mut max_dist_sq = 0.0f32;
  let mut best_pair = (0, polyline.len() / 2);

  for i in 0..polyline.len() {
    for j in i + 1..polyline.len() {
      let dist_sq = (polyline[i] - polyline[j]).length_squared();
      if dist_sq > max_dist_sq {
        max_dist_sq = dist_sq;
        best_pair = (i, j);
      }
    }
  }

  best_pair
}

/// Splits a closed polyline at two indices into two open polylines.
fn split_at_indices(polyline: &[Vec2], i1: usize, i2: usize) -> (Vec<Vec2>, Vec<Vec2>) {
  let (start, end) = if i1 < i2 { (i1, i2) } else { (i2, i1) };

  // First half: from start to end
  let half1: Vec<Vec2> = polyline[start..=end].to_vec();

  // Second half: from end back to start (wrapping around)
  let mut half2: Vec<Vec2> = polyline[end..].to_vec();
  half2.extend_from_slice(&polyline[..=start]);

  (half1, half2)
}

/// Simplifies an open polyline using Douglas-Peucker.
fn simplify_open(polyline: &[Vec2], tolerance: f32) -> Vec<Vec2> {
  if polyline.len() <= 2 {
    return polyline.to_vec();
  }

  let tolerance_sq = tolerance * tolerance;

  // Find the point with maximum distance from the line between first and last
  let first = polyline[0];
  let last = *polyline.last().unwrap();

  let mut max_dist_sq = 0.0f32;
  let mut max_idx = 0;

  for (i, &point) in polyline.iter().enumerate().skip(1).take(polyline.len() - 2) {
    let dist_sq = perpendicular_distance_squared(point, first, last);
    if dist_sq > max_dist_sq {
      max_dist_sq = dist_sq;
      max_idx = i;
    }
  }

  // If the maximum distance exceeds tolerance, recursively simplify
  if max_dist_sq > tolerance_sq {
    // Simplify both halves
    let mut left = simplify_open(&polyline[..=max_idx], tolerance);
    let right = simplify_open(&polyline[max_idx..], tolerance);

    // Remove duplicate point at the junction
    left.pop();
    left.extend(right);
    left
  } else {
    // All intermediate points are within tolerance, keep only endpoints
    vec![first, last]
  }
}

/// Calculates the squared perpendicular distance from a point to a line
/// segment.
fn perpendicular_distance_squared(point: Vec2, line_start: Vec2, line_end: Vec2) -> f32 {
  let line_vec = line_end - line_start;
  let line_len_sq = line_vec.length_squared();

  if line_len_sq < 1e-10 {
    // Degenerate line segment, return distance to start point
    return (point - line_start).length_squared();
  }

  // Project point onto line
  let t = ((point - line_start).dot(line_vec) / line_len_sq).clamp(0.0, 1.0);
  let projection = line_start + t * line_vec;

  (point - projection).length_squared()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_simplify_triangle_unchanged() {
    let triangle = vec![
      Vec2::new(0.0, 0.0),
      Vec2::new(10.0, 0.0),
      Vec2::new(5.0, 10.0),
    ];

    let simplified = douglas_peucker(&triangle, 1.0);
    assert_eq!(simplified.len(), 3, "Triangle should remain unchanged");
  }

  #[test]
  fn test_simplify_reduces_colinear_points() {
    // A line with many colinear points
    let line = vec![
      Vec2::new(0.0, 0.0),
      Vec2::new(1.0, 0.0),
      Vec2::new(2.0, 0.0),
      Vec2::new(3.0, 0.0),
      Vec2::new(4.0, 0.0),
      Vec2::new(4.0, 4.0),
      Vec2::new(0.0, 4.0),
    ];

    let simplified = douglas_peucker(&line, 0.1);
    assert!(
      simplified.len() < line.len(),
      "Colinear points should be reduced"
    );
  }

  #[test]
  fn test_simplify_preserves_sharp_corners() {
    // A square with a spike
    let shape = vec![
      Vec2::new(0.0, 0.0),
      Vec2::new(5.0, 0.0),
      Vec2::new(5.0, 5.0),
      Vec2::new(2.5, 10.0), // Sharp spike
      Vec2::new(0.0, 5.0),
    ];

    let simplified = douglas_peucker(&shape, 1.0);
    // The spike should be preserved because it exceeds tolerance
    assert!(simplified.len() >= 4, "Sharp corners should be preserved");
  }

  #[test]
  fn test_perpendicular_distance() {
    let point = Vec2::new(5.0, 5.0);
    let line_start = Vec2::new(0.0, 0.0);
    let line_end = Vec2::new(10.0, 0.0);

    let dist_sq = perpendicular_distance_squared(point, line_start, line_end);
    assert!(
      (dist_sq - 25.0).abs() < 0.001,
      "Distance should be 5.0 (squared = 25.0)"
    );
  }
}
