//! Noise profile synchronization and NoiseTool IPC integration.
//!
//! This module handles:
//! - Syncing NoiseProfile resource from WorldConfigData yoleck entity
//! - Polling IPC for updates from external NoiseTool editor
//! - Triggering seeder updates when noise configuration changes
//!
//! The actual UI for editing noise settings is in the WorldConfig yoleck
//! entity panel (see `entities/world_config.rs`).

use std::sync::Arc;

use bevy::prelude::*;
use bevy_pixel_world::{MaterialSeeder, UpdateSeeder};
use noise_ipc::NoiseIpc;

use super::entities::WorldConfigData;

/// Current noise profile being edited.
///
/// This resource mirrors the WorldConfigData values for the current level,
/// providing a working copy that syncs with yoleck.
#[derive(Resource)]
pub struct NoiseProfile {
  /// Encoded node tree (ENT) string.
  pub ent: String,
  /// World seed for procedural generation.
  pub world_seed: i32,
  /// Noise threshold for solid/void boundary.
  pub threshold: f32,
  /// Whether the profile has been modified and needs to update the seeder.
  pub dirty: bool,
  /// Entity ID of the WorldConfigData we synced from (for detecting level
  /// changes).
  synced_from: Option<Entity>,
}

impl Default for NoiseProfile {
  fn default() -> Self {
    Self {
      ent: bevy_pixel_world::noise_presets::SIMPLEX.to_string(),
      world_seed: 42,
      threshold: 0.0,
      dirty: false,
      synced_from: None,
    }
  }
}

/// Optional IPC connection to NoiseTool.
#[derive(Resource, Default)]
pub struct NoiseIpcConnection {
  pub client: Option<NoiseIpc>,
}

pub fn setup(app: &mut App) {
  app.init_resource::<NoiseProfile>();
  app.init_resource::<NoiseIpcConnection>();
  app.add_systems(
    Update,
    (sync_profile_from_world_config, poll_ipc_and_apply_changes).chain(),
  );
}

/// Syncs NoiseProfile from WorldConfigData on level load.
///
/// Handles:
/// - Initial sync when WorldConfigData first appears
/// - Re-sync when level changes (different entity)
/// - Reset when level is unloaded (entity gone)
fn sync_profile_from_world_config(
  mut profile: ResMut<NoiseProfile>,
  world_config: Query<(Entity, &WorldConfigData)>,
) {
  match world_config.single() {
    Ok((entity, config)) => {
      // Check if we need to sync (new entity or first time)
      let needs_sync = match profile.synced_from {
        Some(prev_entity) => prev_entity != entity,
        None => true,
      };

      if needs_sync {
        profile.ent = config.noise_ent.clone();
        profile.world_seed = config.world_seed;
        profile.threshold = config.threshold;
        profile.synced_from = Some(entity);
        // Don't set dirty - seeder already matches config on load.
        // Setting dirty here would trigger ReseedAllChunks, overriding
        // ReloadAllChunks from on_enter_playing and breaking persistence.
        info!(
          "Synced noise profile from level: seed={}, threshold={}, ent={}",
          config.world_seed,
          config.threshold,
          &config.noise_ent[..config.noise_ent.len().min(20)]
        );
      }
    }
    Err(_) => {
      // No WorldConfigData - either no level loaded or level lacks WorldConfig
      // Reset sync state so we re-sync when a new level loads
      if profile.synced_from.is_some() {
        profile.synced_from = None;
        debug!("WorldConfigData gone - will re-sync when new level loads");
      }
    }
  }
}

/// Polls IPC for NoiseTool updates and applies changes to the seeder.
fn poll_ipc_and_apply_changes(
  mut profile: ResMut<NoiseProfile>,
  mut ipc: ResMut<NoiseIpcConnection>,
  mut update_seeder: bevy::ecs::message::MessageWriter<UpdateSeeder>,
  mut world_config: Query<&mut WorldConfigData>,
) {
  // Poll IPC for updates from NoiseTool
  if let Some(client) = &mut ipc.client {
    if let Some(new_ent) = client.poll() {
      if new_ent != profile.ent {
        profile.ent = new_ent;
        profile.dirty = true;
        info!("Received ENT update from NoiseTool");
      }
    }
  }

  // Apply changes when profile is dirty
  if profile.dirty {
    // Create and send new seeder
    if let Some(seeder) = MaterialSeeder::from_encoded(&profile.ent, profile.world_seed) {
      let seeder = seeder.threshold(profile.threshold);
      update_seeder.write(UpdateSeeder {
        seeder: Arc::new(seeder),
      });

      // Update WorldConfigData to trigger yoleck dirty flag
      if let Ok(mut config) = world_config.single_mut() {
        config.noise_ent = profile.ent.clone();
        config.world_seed = profile.world_seed;
        config.threshold = profile.threshold;
      }
    } else {
      warn!("Failed to create seeder from ENT: {}", profile.ent);
    }
    profile.dirty = false;
  }
}
