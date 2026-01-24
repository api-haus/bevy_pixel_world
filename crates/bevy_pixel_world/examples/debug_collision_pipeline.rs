//! Visual debug example for collision mesh pipeline.
//!
//! Displays three stages side-by-side:
//! 1. Raw marching squares polylines (yellow)
//! 2. Douglas-Peucker simplified polylines (cyan)
//! 3. CDT triangulated mesh (green)
//!
//! Run with: `cargo run -p bevy_pixel_world --example debug_collision_pipeline`

use bevy::prelude::*;
use bevy_pixel_world::collision::{
  GRID_SIZE, marching_squares, simplify_polylines, triangulate_polygon,
};

fn main() {
  App::new()
    .add_plugins(DefaultPlugins.set(WindowPlugin {
      primary_window: Some(Window {
        title: "Collision Pipeline Debug".to_string(),
        resolution: (1200, 600).into(),
        ..default()
      }),
      ..default()
    }))
    .add_systems(Startup, setup)
    .add_systems(Update, draw_pipeline_stages)
    .run();
}

fn setup(mut commands: Commands) {
  use bevy::camera::ScalingMode;
  // Center camera to show all 3 panels (spans x=0 to x=40, y=10 to y=22)
  commands.spawn((
    Camera2d,
    Transform::from_xyz(20.0, 15.0, 0.0),
    Projection::Orthographic(OrthographicProjection {
      near: -1000.0,
      far: 1000.0,
      scale: 1.0,
      viewport_origin: Vec2::new(0.5, 0.5),
      scaling_mode: ScalingMode::FixedVertical {
        viewport_height: 35.0,
      },
      area: bevy::math::Rect::default(),
    }),
  ));
}

/// Creates a 34x34 grid with a 10x10 solid block in the center.
fn create_test_grid() -> [[bool; GRID_SIZE]; GRID_SIZE] {
  let mut grid = [[false; GRID_SIZE]; GRID_SIZE];
  // Create a 10x10 solid block in center (indices 12..22)
  for y in 12..22 {
    for x in 12..22 {
      grid[y][x] = true;
    }
  }
  grid
}

fn draw_pipeline_stages(mut gizmos: Gizmos) {
  let grid = create_test_grid();

  // Panel offsets - closer together to fit in view
  let panel1_x = 0.0; // Raw marching squares
  let panel2_x = 15.0; // Simplified
  let panel3_x = 30.0; // Triangulated

  // Draw separator lines
  gizmos.line_2d(
    Vec2::new(-5.0, -5.0),
    Vec2::new(-5.0, 30.0),
    Color::srgba(0.3, 0.3, 0.3, 0.5),
  );
  gizmos.line_2d(
    Vec2::new(10.0, -5.0),
    Vec2::new(10.0, 30.0),
    Color::srgba(0.3, 0.3, 0.3, 0.5),
  );
  gizmos.line_2d(
    Vec2::new(25.0, -5.0),
    Vec2::new(25.0, 30.0),
    Color::srgba(0.3, 0.3, 0.3, 0.5),
  );
  gizmos.line_2d(
    Vec2::new(40.0, -5.0),
    Vec2::new(40.0, 30.0),
    Color::srgba(0.3, 0.3, 0.3, 0.5),
  );

  // Stage 1: Marching squares (yellow)
  let raw_polylines = marching_squares(&grid, Vec2::new(panel1_x, 0.0));
  for polyline in &raw_polylines {
    for i in 0..polyline.len() {
      let a = polyline[i];
      let b = polyline[(i + 1) % polyline.len()];
      gizmos.line_2d(a, b, Color::srgb(1.0, 1.0, 0.0));
    }
    for v in polyline {
      gizmos.circle_2d(*v, 0.2, Color::srgb(1.0, 0.5, 0.0));
    }
  }

  // Stage 2: Simplified (cyan)
  let simplified = simplify_polylines(raw_polylines.clone(), 0.5);
  for polyline in &simplified {
    for i in 0..polyline.len() {
      let a = polyline[i] + Vec2::new(panel2_x, 0.0);
      let b = polyline[(i + 1) % polyline.len()] + Vec2::new(panel2_x, 0.0);
      gizmos.line_2d(a, b, Color::srgb(0.0, 1.0, 1.0));
    }
    for v in polyline {
      gizmos.circle_2d(
        *v + Vec2::new(panel2_x, 0.0),
        0.4,
        Color::srgb(0.0, 0.5, 1.0),
      );
    }
  }

  // Stage 3: Triangulated (green)
  for polyline in &simplified {
    let triangles = triangulate_polygon(polyline);
    for tri in &triangles {
      let a = polyline[tri.0] + Vec2::new(panel3_x, 0.0);
      let b = polyline[tri.1] + Vec2::new(panel3_x, 0.0);
      let c = polyline[tri.2] + Vec2::new(panel3_x, 0.0);
      gizmos.line_2d(a, b, Color::srgb(0.2, 0.8, 0.3));
      gizmos.line_2d(b, c, Color::srgb(0.2, 0.8, 0.3));
      gizmos.line_2d(c, a, Color::srgb(0.2, 0.8, 0.3));
    }
    for v in polyline {
      gizmos.circle_2d(
        *v + Vec2::new(panel3_x, 0.0),
        0.4,
        Color::srgb(0.1, 0.6, 0.2),
      );
    }
  }

  // Debug info: print polyline stats once per second (approximately)
  static PRINTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
  if !PRINTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
    println!("=== Collision Pipeline Debug ===");
    println!(
      "Raw polylines: {} (vertices: {:?})",
      raw_polylines.len(),
      raw_polylines.iter().map(|p| p.len()).collect::<Vec<_>>()
    );
    println!(
      "Simplified polylines: {} (vertices: {:?})",
      simplified.len(),
      simplified.iter().map(|p| p.len()).collect::<Vec<_>>()
    );
    for (i, polyline) in simplified.iter().enumerate() {
      println!("Polyline {} vertices: {:?}", i, polyline);
      let triangles = triangulate_polygon(polyline);
      println!(
        "Polyline {}: {} vertices -> {} triangles",
        i,
        polyline.len(),
        triangles.len()
      );
      for (j, tri) in triangles.iter().enumerate() {
        println!(
          "  Triangle {}: indices ({}, {}, {})",
          j, tri.0, tri.1, tri.2
        );
      }
    }
  }
}
