//! Visual debug UI helpers.

use bevy_egui::egui;

use super::settings::VisualDebugSettings;

/// Renders visual debug checkboxes into the given egui UI.
/// Returns true if any setting changed.
pub fn visual_debug_checkboxes(ui: &mut egui::Ui, settings: &mut VisualDebugSettings) -> bool {
  let mut changed = false;

  changed |= ui
    .checkbox(&mut settings.show_collision_meshes, "Collision meshes")
    .changed();
  changed |= ui
    .checkbox(&mut settings.show_chunk_boundaries, "Chunk boundaries")
    .changed();
  changed |= ui
    .checkbox(&mut settings.show_tile_boundaries, "Tile boundaries")
    .changed();
  changed |= ui
    .checkbox(&mut settings.show_dirty_rects, "Dirty rects")
    .changed();
  changed |= ui
    .checkbox(&mut settings.show_blit_rects, "Blit rects")
    .changed();
  changed |= ui
    .checkbox(&mut settings.show_pixel_body_centers, "Pixel body centers")
    .changed();

  changed
}
