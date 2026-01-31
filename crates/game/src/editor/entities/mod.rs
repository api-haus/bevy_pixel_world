mod platform;
mod spawn_point;
mod world_config;

use bevy::prelude::*;
pub use spawn_point::PlayerSpawnPoint;
pub use world_config::WorldConfigData;

/// Register entity types for yoleck (needed for both editor and game).
pub fn register_entity_types(app: &mut App) {
  spawn_point::register(app);
  platform::register(app);
  world_config::register(app);
}

/// Register edit systems (only for editor mode).
#[cfg(feature = "editor")]
pub fn register_edit_systems(app: &mut App) {
  platform::register_edit_systems(app);
  world_config::register_edit_systems(app);
}
