//! Collision mesh caching and async task management.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy::tasks::Task;

use super::mesh::TileCollisionMesh;
use crate::pixel_world::coords::TilePos;

/// Cached collision meshes per tile.
#[derive(Resource, Default)]
pub struct CollisionCache {
  /// Tile position -> cached mesh.
  meshes: HashMap<TilePos, TileCollisionMesh>,
  /// Tiles currently being generated (avoid duplicate tasks).
  in_flight: HashSet<TilePos>,
  /// Global generation counter, incremented on each mesh insert.
  generation: u64,
}

impl CollisionCache {
  /// Returns the cached mesh for a tile, if available.
  pub fn get(&self, tile: TilePos) -> Option<&TileCollisionMesh> {
    self.meshes.get(&tile)
  }

  /// Returns true if a mesh is cached for this tile.
  pub fn contains(&self, tile: TilePos) -> bool {
    self.meshes.contains_key(&tile)
  }

  /// Returns true if this tile has an in-flight generation task.
  pub fn is_in_flight(&self, tile: TilePos) -> bool {
    self.in_flight.contains(&tile)
  }

  /// Marks a tile as having an in-flight generation task.
  pub fn mark_in_flight(&mut self, tile: TilePos) {
    self.in_flight.insert(tile);
  }

  /// Inserts a completed async task result into the cache.
  ///
  /// Returns true if the mesh was inserted, false if the tile was
  /// invalidated while in-flight (and thus the result should be discarded).
  pub fn insert(&mut self, tile: TilePos, mut mesh: TileCollisionMesh) -> bool {
    // Only insert if tile is still in-flight (wasn't invalidated)
    if self.in_flight.remove(&tile) {
      self.generation += 1;
      mesh.generation = self.generation;
      self.meshes.insert(tile, mesh);
      true
    } else {
      // Tile was invalidated while in-flight; discard stale result
      false
    }
  }

  /// Directly inserts a mesh into the cache (for synchronous operations).
  ///
  /// Use this for immediate caching (e.g., empty tiles detected
  /// synchronously). Does NOT check in_flight status.
  pub fn insert_direct(&mut self, tile: TilePos, mut mesh: TileCollisionMesh) {
    self.generation += 1;
    mesh.generation = self.generation;
    self.meshes.insert(tile, mesh);
  }

  /// Invalidates a cached mesh (called when tile becomes dirty).
  pub fn invalidate(&mut self, tile: TilePos) {
    self.meshes.remove(&tile);
    self.in_flight.remove(&tile);
  }

  /// Invalidates all tiles within a chunk.
  ///
  /// This is more efficient than invalidating tiles one by one.
  pub fn invalidate_chunk(&mut self, chunk_x: i32, chunk_y: i32, tiles_per_chunk: u32) {
    let tile_size = tiles_per_chunk as i64;
    let base_tx = chunk_x as i64 * tile_size;
    let base_ty = chunk_y as i64 * tile_size;

    for ty in 0..tile_size {
      for tx in 0..tile_size {
        let tile = TilePos::new(base_tx + tx, base_ty + ty);
        self.meshes.remove(&tile);
        self.in_flight.remove(&tile);
      }
    }
  }

  /// Returns iterator over all cached tile positions.
  pub fn cached_tiles(&self) -> impl Iterator<Item = TilePos> + '_ {
    self.meshes.keys().copied()
  }

  /// Returns the number of cached meshes.
  pub fn len(&self) -> usize {
    self.meshes.len()
  }

  /// Returns true if the cache is empty.
  pub fn is_empty(&self) -> bool {
    self.meshes.is_empty()
  }
}

/// A single in-flight collision generation task.
pub struct CollisionTask {
  /// The tile being generated.
  pub tile: TilePos,
  /// The async task computing the mesh.
  pub task: Task<TileCollisionMesh>,
}

/// Async collision generation tasks.
#[derive(Resource, Default)]
pub struct CollisionTasks {
  /// Active generation tasks.
  pub tasks: Vec<CollisionTask>,
}

impl CollisionTasks {
  /// Spawns a new collision generation task.
  pub fn spawn(&mut self, tile: TilePos, task: Task<TileCollisionMesh>) {
    self.tasks.push(CollisionTask { tile, task });
  }

  /// Returns the number of active tasks.
  pub fn len(&self) -> usize {
    self.tasks.len()
  }

  /// Returns true if there are no active tasks.
  pub fn is_empty(&self) -> bool {
    self.tasks.is_empty()
  }
}
