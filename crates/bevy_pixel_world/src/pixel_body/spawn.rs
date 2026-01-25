//! Spawn command for pixel bodies.
//!
//! Provides a simple API for spawning pixel bodies from image assets.

#[cfg(feature = "avian2d")]
use avian2d::prelude::RigidBody;
use bevy::prelude::*;
#[cfg(feature = "rapier2d")]
use bevy_rapier2d::prelude::RigidBody;

use super::{BlittedTransform, DisplacementState, Persistable, PixelBodyId, PixelBodyLoader};
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
use crate::collision::CollisionQueryPoint;
use crate::coords::MaterialId;
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
use crate::culling::StreamCulled;

/// Resource that generates unique IDs for pixel bodies.
///
/// Uses a simple counter combined with a timestamp seed for uniqueness
/// across sessions.
#[derive(Resource)]
pub struct PixelBodyIdGenerator {
  counter: u64,
  session_seed: u64,
}

impl Default for PixelBodyIdGenerator {
  fn default() -> Self {
    // Use current time as session seed to avoid ID collisions across sessions
    let session_seed = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .map(|d| d.as_nanos() as u64)
      .unwrap_or(0);
    Self {
      counter: 0,
      session_seed,
    }
  }
}

impl PixelBodyIdGenerator {
  /// Generates a new unique pixel body ID.
  pub fn generate(&mut self) -> PixelBodyId {
    self.counter += 1;
    // Combine session seed and counter with XOR and rotation for better
    // distribution
    let id = self.session_seed.wrapping_add(self.counter);
    PixelBodyId::new(id)
  }

  /// Sets the counter to at least the given value.
  ///
  /// Used when loading persisted bodies to avoid ID collisions.
  pub fn ensure_above(&mut self, min_id: u64) {
    if min_id >= self.session_seed {
      let needed_counter = min_id - self.session_seed + 1;
      self.counter = self.counter.max(needed_counter);
    }
  }
}

/// Command to spawn a pixel body from an image asset.
///
/// The image is loaded asynchronously. The pixel body will be spawned once
/// the image is fully loaded.
///
/// # Example
/// ```ignore
/// fn spawn_crate(mut commands: Commands) {
///     commands.queue(SpawnPixelBody::new(
///         "sprites/crate.png",
///         material_ids::WOOD,
///         Vec2::new(100.0, 200.0),
///     ));
/// }
/// ```
pub struct SpawnPixelBody {
  /// Asset path to the image.
  pub path: String,
  /// Material for all pixels in the body.
  pub material: MaterialId,
  /// World position to spawn at.
  pub position: Vec2,
}

impl SpawnPixelBody {
  /// Creates a new spawn command from an asset path.
  ///
  /// The path is relative to the `assets/` folder.
  pub fn new(path: impl Into<String>, material: MaterialId, position: Vec2) -> Self {
    Self {
      path: path.into(),
      material,
      position,
    }
  }
}

/// Command to spawn a pixel body from a pre-loaded image handle.
///
/// Use this when you need more control over asset loading (e.g., loading from
/// a custom asset source or when the image is already loaded).
///
/// # Example
/// ```ignore
/// fn spawn_crate(mut commands: Commands, asset_server: Res<AssetServer>) {
///     let image = asset_server.load("sprites/crate.png");
///     commands.queue(SpawnPixelBodyFromImage::new(
///         image,
///         material_ids::WOOD,
///         Vec2::new(100.0, 200.0),
///     ));
/// }
/// ```
pub struct SpawnPixelBodyFromImage {
  /// Handle to the image.
  pub image: Handle<Image>,
  /// Material for all pixels in the body.
  pub material: MaterialId,
  /// World position to spawn at.
  pub position: Vec2,
}

impl SpawnPixelBodyFromImage {
  /// Creates a new spawn command from a pre-loaded image handle.
  pub fn new(image: Handle<Image>, material: MaterialId, position: Vec2) -> Self {
    Self {
      image,
      material,
      position,
    }
  }
}

impl bevy::ecs::system::Command for SpawnPixelBodyFromImage {
  fn apply(self, world: &mut bevy::ecs::world::World) {
    // Spawn a pending entity with the provided handle
    world.spawn(PendingPixelBody {
      image: self.image,
      material: self.material,
      position: self.position,
    });
  }
}

impl bevy::ecs::system::Command for SpawnPixelBody {
  fn apply(self, world: &mut bevy::ecs::world::World) {
    // Load the image asset
    let asset_server = world.resource::<AssetServer>();
    let image_handle: Handle<Image> = asset_server.load(&self.path);

    // Spawn a pending entity that will be finalized when the image loads
    world.spawn(PendingPixelBody {
      image: image_handle,
      material: self.material,
      position: self.position,
    });
  }
}

/// Marker component for pixel bodies that are waiting for their image to load.
#[derive(Component)]
pub struct PendingPixelBody {
  /// Handle to the image being loaded.
  pub image: Handle<Image>,
  /// Material for all pixels.
  pub material: MaterialId,
  /// World position to spawn at.
  pub position: Vec2,
}

/// System that finalizes pending pixel body spawns when their images are
/// loaded.
///
/// This system should be added to your app when using `SpawnPixelBody`.
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
pub fn finalize_pending_pixel_bodies(
  mut commands: Commands,
  pending: Query<(Entity, &PendingPixelBody)>,
  images: Res<Assets<Image>>,
  mut id_generator: ResMut<PixelBodyIdGenerator>,
) {
  for (entity, pending_body) in pending.iter() {
    let Some(image) = images.get(&pending_body.image) else {
      // Image not loaded yet, skip
      continue;
    };

    // Create pixel body from image
    let Some(body) = PixelBodyLoader::from_image_with_material(image, pending_body.material) else {
      // Failed to create body (empty image?), despawn the pending entity
      commands.entity(entity).despawn();
      continue;
    };

    // Generate collider
    let Some(collider) = super::generate_collider(&body) else {
      // Failed to generate collider, despawn
      commands.entity(entity).despawn();
      continue;
    };

    // Generate unique ID for this body
    let body_id = id_generator.generate();

    // Replace the pending entity with the full pixel body
    commands
      .entity(entity)
      .remove::<PendingPixelBody>()
      .insert((
        body,
        collider,
        RigidBody::Dynamic,
        CollisionQueryPoint,
        StreamCulled,
        BlittedTransform::default(),
        DisplacementState::default(),
        Transform::from_translation(pending_body.position.extend(0.0)),
        body_id,
        Persistable,
      ));
  }
}

/// Stub system for when no physics feature is enabled.
#[cfg(not(any(feature = "avian2d", feature = "rapier2d")))]
pub fn finalize_pending_pixel_bodies(
  mut commands: Commands,
  pending: Query<(Entity, &PendingPixelBody)>,
  images: Res<Assets<Image>>,
  mut id_generator: ResMut<PixelBodyIdGenerator>,
) {
  for (entity, pending_body) in pending.iter() {
    let Some(image) = images.get(&pending_body.image) else {
      continue;
    };

    // Create pixel body from image (no collider without physics)
    let Some(body) = PixelBodyLoader::from_image_with_material(image, pending_body.material) else {
      commands.entity(entity).despawn();
      continue;
    };

    // Generate unique ID for this body
    let body_id = id_generator.generate();

    commands
      .entity(entity)
      .remove::<PendingPixelBody>()
      .insert((
        body,
        BlittedTransform::default(),
        DisplacementState::default(),
        Transform::from_translation(pending_body.position.extend(0.0)),
        body_id,
        Persistable,
      ));
  }
}
