//! Palettization of player sprites.

use std::collections::HashSet;

use bevy::prelude::*;

use super::components::PlayerVisual;
use crate::pixel_world::{GlobalPalette, palettize_image_in_place};

/// Tracks which image handles have been palettized to avoid re-processing.
#[derive(Default)]
pub struct PalettizedImages {
  handles: HashSet<AssetId<Image>>,
}

/// System that palettizes the player sprite when the image is loaded.
pub fn palettize_player_sprite(
  player_visuals: Query<&Sprite, With<PlayerVisual>>,
  mut images: ResMut<Assets<Image>>,
  palette: Option<Res<GlobalPalette>>,
  mut palettized: Local<PalettizedImages>,
) {
  let Some(palette) = palette else {
    return;
  };

  for sprite in &player_visuals {
    let handle = &sprite.image;
    let id = handle.id();

    // Skip if already palettized
    if palettized.handles.contains(&id) {
      continue;
    }

    // Check if the image is loaded
    if let Some(image) = images.get_mut(id) {
      palettize_image_in_place(image, &palette);
      palettized.handles.insert(id);
      info!("Palettized player sprite");
    }
  }
}
