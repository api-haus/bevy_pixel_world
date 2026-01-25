//! Bevy systems for collision mesh generation.

use bevy::math::Vec2;
use bevy::prelude::*;
use bevy::tasks::AsyncComputeTaskPool;

use super::CollisionConfig;
use super::cache::{CollisionCache, CollisionTasks};
use super::marching::{GRID_SIZE, marching_squares};
use super::mesh::{PolygonMesh, TileCollisionMesh};
use super::simplify::simplify_polylines;
use super::triangulate::triangulate_polygon;
use crate::coords::{TILE_SIZE, TILES_PER_CHUNK, TilePos};
use crate::material::{Materials, PhysicsState};
use crate::pixel::PixelFlags;
use crate::world::PixelWorld;

/// Marker component for entities that trigger collision mesh generation.
///
/// Attach this to any entity (typically the player or camera) to generate
/// collision meshes around its position.
#[derive(Component, Default)]
pub struct CollisionQueryPoint;

/// Returns tiles within a square radius around the center tile.
fn tiles_in_radius(center: TilePos, radius: u32) -> impl Iterator<Item = TilePos> {
  let r = radius as i64;
  (-r..=r).flat_map(move |dy| (-r..=r).map(move |dx| TilePos::new(center.x + dx, center.y + dy)))
}

/// Converts a world position to a tile position.
fn world_to_tile(world_pos: Vec2) -> TilePos {
  let tile_size = TILE_SIZE as f32;
  TilePos::new(
    (world_pos.x / tile_size).floor() as i64,
    (world_pos.y / tile_size).floor() as i64,
  )
}

/// Extracts a 34x34 binary grid for a tile, including 1px border from
/// neighbors.
///
/// Returns a grid where `true` indicates a collision pixel.
/// A pixel is considered collision if:
/// - It's not air
/// - Its material is Solid or Powder (settled powders form collision surfaces)
fn extract_tile_grid(
  world: &PixelWorld,
  tile: TilePos,
  materials: &Materials,
) -> [[bool; GRID_SIZE]; GRID_SIZE] {
  let mut grid = [[false; GRID_SIZE]; GRID_SIZE];
  let tile_size = TILE_SIZE as i64;

  // The tile origin in world coordinates
  let tile_origin_x = tile.x * tile_size;
  let tile_origin_y = tile.y * tile_size;

  // Sample a 34x34 area: the 32x32 tile plus 1px border on each side
  for (gy, row) in grid.iter_mut().enumerate() {
    for (gx, cell) in row.iter_mut().enumerate() {
      // Grid position to world position (with 1px border offset)
      let world_x = tile_origin_x + (gx as i64) - 1;
      let world_y = tile_origin_y + (gy as i64) - 1;

      let pos = crate::coords::WorldPos::new(world_x, world_y);

      if let Some(pixel) = world.get_pixel(pos) {
        if pixel.is_void() {
          continue;
        }
        if pixel.flags.contains(PixelFlags::PIXEL_BODY) {
          continue;
        }

        let material = materials.get(pixel.material);
        // Solid and Powder materials form collision surfaces
        // Liquids and gases do not
        *cell = matches!(material.state, PhysicsState::Solid | PhysicsState::Powder);
      }
    }
  }

  grid
}

/// Handles an empty collision tile by caching a default mesh.
fn handle_empty_collision_tile(
  cache: &mut CollisionCache,
  world: &mut PixelWorld,
  tile: TilePos,
  tiles_per_chunk: i64,
) {
  cache.insert_direct(tile, TileCollisionMesh::default());
  clear_tile_dirty(world, tile, tiles_per_chunk);
}

/// Spawns an async task to generate collision mesh for a tile.
fn spawn_collision_mesh_task(
  tasks: &mut CollisionTasks,
  cache: &mut CollisionCache,
  world: &mut PixelWorld,
  grid: [[bool; GRID_SIZE]; GRID_SIZE],
  tile: TilePos,
  tolerance: f32,
  tiles_per_chunk: i64,
) {
  let task_pool = AsyncComputeTaskPool::get();
  let tile_origin = Vec2::new(
    (tile.x * TILE_SIZE as i64) as f32,
    (tile.y * TILE_SIZE as i64) as f32,
  );

  let task = task_pool.spawn(async move {
    #[cfg(feature = "diagnostics")]
    let start = std::time::Instant::now();

    let contours = marching_squares(&grid, tile_origin);
    let simplified = simplify_polylines(contours, tolerance);

    let triangles: Vec<PolygonMesh> = simplified
      .iter()
      .filter(|p| p.len() >= 3)
      .map(|polygon| {
        let indices = triangulate_polygon(polygon);
        PolygonMesh {
          vertices: polygon.clone(),
          indices,
        }
      })
      .collect();

    TileCollisionMesh {
      polylines: simplified,
      triangles,
      generation: 0, // Set by cache on insert
      #[cfg(feature = "diagnostics")]
      generation_time_ms: start.elapsed().as_secs_f32() * 1000.0,
    }
  });

  cache.mark_in_flight(tile);
  tasks.spawn(tile, task);
  clear_tile_dirty(world, tile, tiles_per_chunk);
}

/// Returns true if the grid contains any collision pixels.
fn grid_has_collision(grid: &[[bool; GRID_SIZE]; GRID_SIZE]) -> bool {
  grid.iter().any(|row| row.iter().any(|&v| v))
}

/// System: Dispatches async collision generation tasks for dirty tiles near
/// query points.
pub fn dispatch_collision_tasks(
  mut tasks: ResMut<CollisionTasks>,
  mut cache: ResMut<CollisionCache>,
  mut worlds: Query<&mut PixelWorld>,
  query_points: Query<&Transform, With<CollisionQueryPoint>>,
  config: Res<CollisionConfig>,
  materials: Option<Res<Materials>>,
) {
  let Some(materials) = materials else {
    return;
  };

  let tiles_per_chunk = TILES_PER_CHUNK as i64;

  for mut world in worlds.iter_mut() {
    for transform in query_points.iter() {
      let center = world_to_tile(transform.translation.truncate());

      for tile in tiles_in_radius(center, config.proximity_radius) {
        if cache.contains(tile) || cache.is_in_flight(tile) {
          continue;
        }

        let grid = extract_tile_grid(&world, tile, &materials);

        if !grid_has_collision(&grid) {
          handle_empty_collision_tile(&mut cache, &mut world, tile, tiles_per_chunk);
          continue;
        }

        spawn_collision_mesh_task(
          &mut tasks,
          &mut cache,
          &mut world,
          grid,
          tile,
          config.simplification_tolerance,
          tiles_per_chunk,
        );
      }
    }
  }
}

/// Clears the collision dirty flag for a world tile.
fn clear_tile_dirty(world: &mut PixelWorld, tile: TilePos, tiles_per_chunk: i64) {
  // Convert world tile to chunk + local tile
  let chunk_x = tile.x.div_euclid(tiles_per_chunk) as i32;
  let chunk_y = tile.y.div_euclid(tiles_per_chunk) as i32;
  let tx = tile.x.rem_euclid(tiles_per_chunk) as u32;
  let ty = tile.y.rem_euclid(tiles_per_chunk) as u32;

  let chunk_pos = crate::coords::ChunkPos::new(chunk_x, chunk_y);
  if let Some(chunk) = world.get_chunk_mut(chunk_pos) {
    chunk.clear_tile_collision_dirty(tx, ty);
  }
}

/// System: Polls completed collision generation tasks and caches the results.
pub fn poll_collision_tasks(
  mut tasks: ResMut<CollisionTasks>,
  mut cache: ResMut<CollisionCache>,
  #[cfg(feature = "diagnostics")] mut metrics: ResMut<crate::diagnostics::CollisionMetrics>,
) {
  #[cfg(feature = "diagnostics")]
  let mut completed = 0u32;
  #[cfg(feature = "diagnostics")]
  let mut total_generation_time_ms = 0.0f32;

  tasks.tasks.retain_mut(|task| {
    if !task.task.is_finished() {
      return true; // Keep pending tasks
    }

    let mesh = bevy::tasks::block_on(&mut task.task);
    #[cfg(feature = "diagnostics")]
    {
      total_generation_time_ms += mesh.generation_time_ms;
      completed += 1;
    }
    cache.insert(task.tile, mesh);

    false // Remove completed task
  });

  #[cfg(feature = "diagnostics")]
  {
    metrics.generation_time.push(total_generation_time_ms);
    metrics.tasks_completed.push(completed as f32);
  }
}

/// System: Draws collision meshes as debug gizmos.
#[cfg(feature = "visual_debug")]
pub fn draw_collision_gizmos(
  cache: Res<CollisionCache>,
  query_points: Query<&Transform, With<CollisionQueryPoint>>,
  config: Res<CollisionConfig>,
  mut gizmos: Gizmos,
) {
  if !config.debug_gizmos {
    return;
  }

  // Green color for collision mesh edges
  let edge_color = Color::srgb(0.2, 0.8, 0.3);

  for transform in query_points.iter() {
    let world_pos = transform.translation.truncate();
    let center = world_to_tile(world_pos);

    for tile in tiles_in_radius(center, config.proximity_radius) {
      if let Some(mesh) = cache.get(tile) {
        // Draw triangle edges only
        for polygon_mesh in &mesh.triangles {
          for triangle in &polygon_mesh.indices {
            let a = polygon_mesh.vertices[triangle.a];
            let b = polygon_mesh.vertices[triangle.b];
            let c = polygon_mesh.vertices[triangle.c];

            gizmos.line_2d(a, b, edge_color);
            gizmos.line_2d(b, c, edge_color);
            gizmos.line_2d(c, a, edge_color);
          }
        }
      }
    }
  }
}

/// System: Invalidates collision cache for tiles that have been modified.
///
/// Uses per-tile collision dirty flags from chunks for efficient invalidation.
/// Note: Does not clear the dirty flags - that happens when tasks are spawned.
pub fn invalidate_dirty_tiles(mut cache: ResMut<CollisionCache>, worlds: Query<&PixelWorld>) {
  let tiles_per_chunk = TILES_PER_CHUNK as i64;

  for world in worlds.iter() {
    for (chunk_pos, slot_idx) in world.active_chunks() {
      let slot = world.slot(slot_idx);
      if !slot.is_seeded() {
        continue;
      }

      // Check for dirty tiles in this chunk
      for (tx, ty) in slot.chunk.collision_dirty_tiles() {
        // Convert chunk-local tile to world tile position
        let world_tx = chunk_pos.x as i64 * tiles_per_chunk + tx as i64;
        let world_ty = chunk_pos.y as i64 * tiles_per_chunk + ty as i64;
        let tile_pos = TilePos::new(world_tx, world_ty);

        // Invalidate cache entry
        cache.invalidate(tile_pos);
      }
    }
  }
}

/// Shape types for sample mesh.
#[cfg(feature = "visual_debug")]
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum SampleShapeType {
  #[default]
  Hexagon,
  Star,
  LShape,
}

/// Resource holding a sample mesh for visualization testing.
#[cfg(feature = "visual_debug")]
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

#[cfg(feature = "visual_debug")]
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
#[cfg(feature = "visual_debug")]
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
#[cfg(feature = "visual_debug")]
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
#[cfg(feature = "visual_debug")]
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
