//! Spawn command for pixel bodies.
//!
//! Provides a simple API for spawning pixel bodies from image assets.

use bevy::prelude::*;
#[cfg(physics)]
use bevy_rapier2d::prelude::Collider;

use super::{DisplacementState, LastBlitTransform, Persistable, PixelBodyId, PixelBodyLoader};
#[cfg(physics)]
use crate::pixel_world::collision::CollisionQueryPoint;
use crate::pixel_world::coords::MaterialId;
use crate::pixel_world::palette::GlobalPalette;
#[cfg(physics)]
use crate::pixel_world::world::streaming::culling::StreamCulled;

/// Returns the physics bundle for a pixel body (collider + rigid body +
/// markers).
#[cfg(physics)]
fn physics_bundle(collider: Collider) -> impl Bundle {
  (
    collider,
    bevy_rapier2d::prelude::RigidBody::Dynamic,
    CollisionQueryPoint,
    StreamCulled,
  )
}

/// Returns the damping bundle for submergence physics effects.
#[cfg(physics)]
fn submergence_damping_bundle() -> impl Bundle {
  (
    bevy_rapier2d::prelude::GravityScale(1.0),
    bevy_rapier2d::prelude::Damping {
      linear_damping: 0.0,
      angular_damping: 0.0,
    },
  )
}

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
    #[cfg(not(target_family = "wasm"))]
    let session_seed = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .map(|d| d.as_nanos() as u64)
      .unwrap_or(0);
    #[cfg(target_family = "wasm")]
    let session_seed = (js_sys::Date::now() * 1000.0) as u64;
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
  /// Extra components to insert on the spawned entity.
  extra: Option<Box<dyn FnOnce(&mut bevy::ecs::world::EntityWorldMut) + Send + Sync>>,
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
      extra: None,
    }
  }

  /// Adds extra components to the spawned entity.
  ///
  /// The closure receives a mutable reference to the entity and can insert
  /// any additional components. These are preserved when the pending body
  /// is finalized into a full pixel body.
  ///
  /// # Example
  /// ```ignore
  /// commands.queue(SpawnPixelBody::new("box.png", material_ids::WOOD, pos)
  ///     .with_extra(|entity| {
  ///         entity.insert(Bomb::default());
  ///     }));
  /// ```
  pub fn with_extra<F>(mut self, f: F) -> Self
  where
    F: FnOnce(&mut bevy::ecs::world::EntityWorldMut) + Send + Sync + 'static,
  {
    self.extra = Some(Box::new(f));
    self
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
    let mut entity = world.spawn(PendingPixelBody {
      image: image_handle,
      material: self.material,
      position: self.position,
    });

    // Apply extra components if provided
    if let Some(extra) = self.extra {
      extra(&mut entity);
    }
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
pub fn finalize_pending_pixel_bodies(
  mut commands: Commands,
  pending: Query<(Entity, &PendingPixelBody)>,
  images: Option<Res<Assets<Image>>>,
  palette: Option<Res<GlobalPalette>>,
  mut id_generator: ResMut<PixelBodyIdGenerator>,
) {
  let Some(images) = images else { return };
  let Some(palette) = palette else { return };
  for (entity, pending_body) in pending.iter() {
    let Some(image) = images.get(&pending_body.image) else {
      // Image not loaded yet, skip
      continue;
    };

    // Create pixel body from image using global palette for color mapping
    let Some(body) =
      PixelBodyLoader::from_image_with_material(image, pending_body.material, &palette)
    else {
      commands.entity(entity).despawn();
      continue;
    };

    // Generate collider (physics only)
    #[cfg(physics)]
    let Some(collider) = super::generate_collider(&body) else {
      commands.entity(entity).despawn();
      continue;
    };

    let body_id = id_generator.generate();

    // Replace pending entity with full pixel body
    let mut entity_commands = commands.entity(entity);
    let translation = pending_body.position.extend(0.0);
    entity_commands.remove::<PendingPixelBody>().insert((
      body,
      LastBlitTransform::default(),
      DisplacementState::default(),
      Transform::from_translation(translation),
      // Explicit GlobalTransform ensures correct position on first frame.
      // Without this, GlobalTransform defaults to identity and Bevy's
      // transform propagation doesn't run until PostUpdate - after our
      // blit system, causing bodies to appear at (0,0) initially.
      GlobalTransform::from_translation(translation),
      body_id,
      Persistable,
    ));

    #[cfg(physics)]
    entity_commands.insert(physics_bundle(collider));

    entity_commands.insert(crate::pixel_world::buoyancy::Submergent);

    #[cfg(physics)]
    entity_commands.insert(submergence_damping_bundle());
  }
}
