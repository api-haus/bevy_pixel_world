//! Reusable egui UI for brush controls.

use bevy::prelude::*;
use bevy_egui::{EguiPrimaryContextPass, egui};

use crate::pixel_world::debug_controller::{BrushState, MAX_RADIUS, MIN_RADIUS};
use crate::pixel_world::material::Materials;
use crate::pixel_world::material_ids;

/// All materials available in the brush dropdown.
const BRUSH_MATERIALS: &[(crate::pixel_world::MaterialId, &str)] = &[
  (material_ids::VOID, "Void"),
  (material_ids::SOIL, "Soil"),
  (material_ids::STONE, "Stone"),
  (material_ids::SAND, "Sand"),
  (material_ids::WATER, "Water"),
  (material_ids::WOOD, "Wood"),
  (material_ids::ASH, "Ash"),
];

/// Renders brush controls into an egui UI.
/// Returns true if any setting changed.
pub fn brush_controls_ui(
  ui: &mut egui::Ui,
  brush: &mut BrushState,
  _materials: &Materials,
) -> bool {
  let mut changed = false;

  // Material dropdown
  ui.label("Material");
  let current_name = BRUSH_MATERIALS
    .iter()
    .find(|(id, _)| *id == brush.material)
    .map(|(_, name)| *name)
    .unwrap_or("Unknown");

  egui::ComboBox::from_id_salt("brush_material")
    .selected_text(current_name)
    .show_ui(ui, |ui| {
      for (id, name) in BRUSH_MATERIALS {
        if ui.selectable_label(brush.material == *id, *name).clicked() {
          brush.material = *id;
          changed = true;
        }
      }
    });

  ui.add_space(8.0);

  // Radius slider
  ui.label(format!("Radius: {}", brush.radius));
  let mut radius = brush.radius as i32;
  if ui
    .add(egui::Slider::new(&mut radius, MIN_RADIUS as i32..=MAX_RADIUS as i32).show_value(false))
    .changed()
  {
    brush.radius = radius as u32;
    changed = true;
  }

  ui.add_space(8.0);

  // Heat painting toggle
  if ui
    .checkbox(&mut brush.heat_painting, "Heat painting")
    .changed()
  {
    changed = true;
  }

  if brush.heat_painting {
    ui.label(format!("Heat value: {}", brush.heat_value));
    let mut heat = brush.heat_value as i32;
    if ui
      .add(egui::Slider::new(&mut heat, 0..=255).show_value(false))
      .changed()
    {
      brush.heat_value = heat as u8;
      changed = true;
    }
  }

  changed
}

/// Resource controlling brush UI visibility.
#[derive(Resource, Default)]
pub struct BrushUiVisible(pub bool);

/// Plugin that shows a brush control panel when `BrushUiVisible(true)`.
pub struct BrushUiPlugin;

impl Plugin for BrushUiPlugin {
  fn build(&self, app: &mut App) {
    app.init_resource::<BrushUiVisible>().add_systems(
      EguiPrimaryContextPass,
      show_brush_panel.run_if(|visible: Option<Res<BrushUiVisible>>| visible.is_some_and(|v| v.0)),
    );
  }
}

fn show_brush_panel(
  mut contexts: bevy_egui::EguiContexts,
  mut brush: ResMut<BrushState>,
  materials: Res<Materials>,
) {
  let Ok(ctx) = contexts.ctx_mut() else { return };
  egui::SidePanel::left("brush_panel")
    .default_width(180.0)
    .show(ctx, |ui| {
      ui.heading("Brush");
      ui.separator();
      brush_controls_ui(ui, &mut brush, &materials);
    });
}
