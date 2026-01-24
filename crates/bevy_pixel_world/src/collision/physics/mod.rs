//! Physics library integration for collision meshes.
//!
//! Provides optional feature-gated support for avian2d and rapier2d physics engines.
//! Enable one (but not both) via Cargo features:
//!
//! ```toml
//! bevy_pixel_world = { version = "...", features = ["avian2d"] }
//! # or
//! bevy_pixel_world = { version = "...", features = ["rapier2d"] }
//! ```

#[cfg(all(feature = "avian2d", feature = "rapier2d"))]
compile_error!("Cannot enable both avian2d and rapier2d features simultaneously");

#[cfg(feature = "avian2d")]
pub mod avian;

#[cfg(feature = "rapier2d")]
pub mod rapier;

use std::collections::HashMap;

use bevy::prelude::*;

use crate::coords::TilePos;

/// Tracks spawned physics collider entities by tile position.
#[derive(Resource, Default)]
pub struct PhysicsColliderRegistry {
    pub entities: HashMap<TilePos, Entity>,
}

/// Marker component for tile collider entities.
#[derive(Component)]
pub struct TileCollider {
    pub tile: TilePos,
    /// Generation of the mesh when this collider was created.
    /// Used to detect when the collider needs regeneration.
    pub generation: u64,
}
