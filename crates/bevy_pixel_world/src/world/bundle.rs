//! ECS bundles and commands for spawning PixelWorld entities.

use std::sync::Arc;

use bevy::prelude::*;

use super::{PixelWorld, PixelWorldConfig};
use crate::seeding::ChunkSeeder;

/// Bundle for spawning a PixelWorld entity.
#[derive(Bundle)]
pub struct PixelWorldBundle {
  /// The pixel world component.
  pub world: PixelWorld,
  /// Transform (typically at origin).
  pub transform: Transform,
  /// Global transform (computed by Bevy).
  pub global_transform: GlobalTransform,
}

impl PixelWorldBundle {
  /// Creates a new PixelWorld bundle.
  pub fn new(seeder: impl ChunkSeeder + 'static, mesh: Handle<Mesh>) -> Self {
    Self {
      world: PixelWorld::new(Arc::new(seeder), mesh),
      transform: Transform::default(),
      global_transform: GlobalTransform::default(),
    }
  }
}

/// Command to spawn a PixelWorld using the shared chunk mesh.
///
/// This is the simplest way to create a PixelWorld - just provide a seeder
/// and queue this command. The plugin's SharedChunkMesh is used automatically.
///
/// Uses the default configuration from `PixelWorldPlugin` unless overridden
/// with `with_config()`.
///
/// Chunk loading from persistence is handled by the streaming system
/// (`dispatch_chunk_loads` and `seed_chunk_with_loaded`), not the seeder.
///
/// # Example
/// ```ignore
/// fn setup(mut commands: Commands) {
///     commands.queue(SpawnPixelWorld::new(MaterialSeeder::new(42)));
/// }
/// ```
///
/// # Panics
/// Panics if `PixelWorldPlugin` hasn't been added (SharedChunkMesh not found).
pub struct SpawnPixelWorld {
  seeder: Arc<dyn ChunkSeeder + Send + Sync>,
  config: Option<PixelWorldConfig>,
}

impl SpawnPixelWorld {
  pub fn new(seeder: impl ChunkSeeder + 'static) -> Self {
    Self {
      seeder: Arc::new(seeder),
      config: None,
    }
  }

  /// Sets the world configuration, overriding the plugin default.
  pub fn with_config(mut self, config: PixelWorldConfig) -> Self {
    self.config = Some(config);
    self
  }
}

impl bevy::ecs::system::Command for SpawnPixelWorld {
  fn apply(self, world: &mut bevy::ecs::world::World) {
    let mesh = world
      .get_resource::<super::plugin::SharedChunkMesh>()
      .map(|r| r.0.clone())
      .unwrap_or_default();

    // Use explicit config or fall back to plugin default
    let config = self.config.unwrap_or_else(|| {
      world
        .get_resource::<crate::DefaultPixelWorldConfig>()
        .map(|r| r.0.clone())
        .unwrap_or_default()
    });

    // Persistence loading is handled by dispatch_chunk_loads and
    // seed_chunk_with_loaded, so we use the seeder directly without wrapping.
    world.spawn(PixelWorldBundle {
      world: PixelWorld::with_config(self.seeder, mesh, config),
      transform: Transform::default(),
      global_transform: GlobalTransform::default(),
    });
  }
}
