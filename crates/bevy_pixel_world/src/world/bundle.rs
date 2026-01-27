//! ECS bundles and commands for spawning PixelWorld entities.

use std::sync::Arc;

use bevy::prelude::*;

use super::{PixelWorld, PixelWorldConfig};
use crate::persistence::WorldSaveResource;
use crate::seeding::{ChunkSeeder, PersistenceSeeder};

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
/// When a `WorldSaveResource` exists (persistence enabled), the seeder is
/// automatically wrapped with `PersistenceSeeder` to load saved chunks.
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

    // Wrap seeder with PersistenceSeeder if save resource exists
    let seeder: Arc<dyn ChunkSeeder + Send + Sync> =
      if let Some(save_resource) = world.get_resource::<WorldSaveResource>() {
        let chunk_count = save_resource
          .save
          .read()
          .map(|s| s.chunk_count())
          .unwrap_or(0);
        info!(
          "PixelWorld spawned with persistence ({} saved chunks)",
          chunk_count
        );
        Arc::new(PersistenceSeeder::new(
          ArcSeeder(self.seeder),
          save_resource.save.clone(),
        ))
      } else {
        self.seeder
      };

    world.spawn(PixelWorldBundle {
      world: PixelWorld::with_config(seeder, mesh, config),
      transform: Transform::default(),
      global_transform: GlobalTransform::default(),
    });
  }
}

/// Wrapper to make Arc<dyn ChunkSeeder> implement ChunkSeeder.
struct ArcSeeder(Arc<dyn ChunkSeeder + Send + Sync>);

impl ChunkSeeder for ArcSeeder {
  fn seed(&self, pos: crate::ChunkPos, chunk: &mut crate::Chunk) {
    self.0.seed(pos, chunk);
  }
}
