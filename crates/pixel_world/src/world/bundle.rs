//! ECS bundles and commands for spawning PixelWorld entities.

use std::sync::Arc;

use bevy::prelude::*;

use crate::seeding::ChunkSeeder;

use super::{PixelWorld, PixelWorldConfig};

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
  pub fn new(seeder: impl ChunkSeeder + Send + Sync + 'static, mesh: Handle<Mesh>) -> Self {
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
  pub fn new(seeder: impl ChunkSeeder + Send + Sync + 'static) -> Self {
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
      .expect("SharedChunkMesh not found - add PixelWorldPlugin first")
      .0
      .clone();

    // Use explicit config or fall back to plugin default
    let config = self.config.unwrap_or_else(|| {
      world
        .get_resource::<crate::DefaultPixelWorldConfig>()
        .map(|r| r.0.clone())
        .unwrap_or_default()
    });

    world.spawn(PixelWorldBundle {
      world: PixelWorld::with_config(self.seeder, mesh, config),
      transform: Transform::default(),
      global_transform: GlobalTransform::default(),
    });
  }
}
