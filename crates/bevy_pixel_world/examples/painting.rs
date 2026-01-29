//! Brush Painting Demo - PixelWorld painting and simulation.
//!
//! Demonstrates using the PixelWorld API for pixel modification with
//! cellular automata physics simulation.
//!
//! Controls:
//! - LMB: Paint with selected material
//! - RMB: Erase (paint with void)
//! - Scroll wheel: Adjust brush radius
//! - WASD/Arrow keys: Move camera
//! - Shift: Speed boost (5x)
//! - Space: Spawn random pixel body at cursor (requires avian2d or rapier2d
//!   feature)
//! - Ctrl+S: Manual save
//! - Side panel: Material selection, brush size slider
//!
//! Run with: `cargo run -p bevy_pixel_world --example painting`
//! With physics: `cargo run -p bevy_pixel_world --example painting --features
//! avian2d`

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
use bevy_pixel_world::SpawnPixelBody;
use bevy_pixel_world::buoyancy::SubmersionState;
use bevy_pixel_world::debug_camera::CameraZoom;
use bevy_pixel_world::debug_controller::{BrushState, MAX_RADIUS, MIN_RADIUS, UiPointerState};
use bevy_pixel_world::visual_debug::{
  SettingsPersistence, VisualDebugSettings, visual_debug_checkboxes,
};
use bevy_pixel_world::{
  Bomb, CreativeModePlugins, MaterialSeeder, Materials, MaterialsConfig, PersistenceConfig,
  PixelBody, PixelFlags, PixelWorld, PixelWorldFullBundle, SpawnPixelWorld, WorldPos, material_ids,
};
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
use rand::Rng;

fn main() {
  let config: MaterialsConfig = toml::from_str(include_str!("materials.toml")).unwrap();

  // Compute save path in user's data directory
  let save_path = dirs::data_dir()
    .unwrap_or_else(|| std::path::PathBuf::from("."))
    .join("pixel_world_painting")
    .join("saves")
    .join("world.save");

  App::new()
    .add_plugins(DefaultPlugins.set(WindowPlugin {
      primary_window: Some(Window {
        title: "Brush Painting Demo - PixelWorld".to_string(),
        resolution: (1280, 720).into(),
        ..default()
      }),
      ..default()
    }))
    .insert_resource(Materials::from(config))
    .add_plugins(PixelWorldFullBundle::default().persistence(PersistenceConfig::at(save_path)))
    .add_plugins((CreativeModePlugins, UiPlugin, PhysicsPlugin))
    .add_systems(Startup, setup)
    .run();
}

fn setup(mut commands: Commands) {
  commands.queue(SpawnPixelWorld::new(MaterialSeeder::new(42)));
}

// ─── UiPlugin ────────────────────────────────────────────────────────────────

struct UiPlugin;

impl Plugin for UiPlugin {
  fn build(&self, app: &mut App) {
    app
      .init_resource::<UiPointerState>()
      .add_systems(EguiPrimaryContextPass, ui_system);
  }
}

#[allow(unused_mut, unused_variables)]
fn ui_system(
  mut contexts: EguiContexts,
  mut brush: ResMut<BrushState>,
  mut zoom: ResMut<CameraZoom>,
  materials: Res<Materials>,
  mut ui_state: ResMut<UiPointerState>,
  worlds: Query<&PixelWorld>,
  body_query: Query<Entity, With<PixelBody>>,
  submersion_query: Query<&SubmersionState>,
  mut settings: ResMut<VisualDebugSettings>,
  mut persistence: ResMut<SettingsPersistence>,
) {
  let Ok(ctx) = contexts.ctx_mut() else {
    return;
  };

  let cursor_pixel = brush.world_pos.and_then(|(x, y)| {
    worlds
      .single()
      .ok()
      .and_then(|world| world.get_pixel(WorldPos::new(x, y)).copied())
  });

  egui::SidePanel::left("tools_panel")
    .resizable(true)
    .default_width(180.0)
    .width_range(150.0..=400.0)
    .frame(
      egui::Frame::NONE
        .fill(egui::Color32::from_rgba_unmultiplied(20, 20, 25, 200))
        .inner_margin(8.0),
    )
    .show(ctx, |ui| {
      // Brush section
      egui::CollapsingHeader::new("Brush")
        .default_open(true)
        .show(ui, |ui| {
          for id in [
            material_ids::SOIL,
            material_ids::STONE,
            material_ids::SAND,
            material_ids::WATER,
            material_ids::WOOD,
            material_ids::ASH,
          ] {
            let mat = materials.get(id);
            if ui
              .selectable_label(brush.material == id, mat.name)
              .clicked()
            {
              brush.material = id;
            }
          }

          ui.separator();

          let mut radius = brush.radius as f32;
          ui.add(
            egui::Slider::new(&mut radius, MIN_RADIUS as f32..=MAX_RADIUS as f32).text("Size"),
          );
          brush.radius = radius as u32;

          ui.separator();

          ui.checkbox(&mut brush.heat_painting, "Heat brush");
          if brush.heat_painting {
            let mut heat = brush.heat_value as f32;
            ui.add(egui::Slider::new(&mut heat, 0.0..=255.0).text("Heat"));
            brush.heat_value = heat as u8;
          }
        });

      // Camera section
      egui::CollapsingHeader::new("Camera")
        .default_open(true)
        .show(ui, |ui| {
          ui.label(format!(
            "Viewport: {}x{}",
            zoom.width as i32, zoom.height as i32
          ));

          ui.horizontal(|ui| {
            if ui.button("−").clicked() {
              zoom.zoom_out();
            }
            ui.label("Zoom");
            if ui.button("+").clicked() {
              zoom.zoom_in();
            }
          });

          ui.separator();
          ui.label("Presets:");

          for chunk in CameraZoom::PRESETS.chunks(2) {
            ui.horizontal(|ui| {
              for &(w, h, label) in chunk {
                if ui.button(label).clicked() {
                  zoom.width = w;
                  zoom.height = h;
                }
              }
            });
          }
        });

      // Pixel Debug section
      egui::CollapsingHeader::new("Pixel Debug")
        .default_open(true)
        .show(ui, |ui| {
          if let Some((x, y)) = brush.world_pos {
            ui.label(format!("Position: ({}, {})", x, y));

            if let Some(pixel) = cursor_pixel {
              ui.separator();

              let mat_name = if pixel.material.0 == 0 {
                "VOID"
              } else {
                materials.get(pixel.material).name
              };
              ui.label(format!("Material: {} ({})", mat_name, pixel.material.0));
              ui.label(format!("Color: {}", pixel.color.0));
              ui.label(format!("Damage: {}", pixel.damage));

              ui.separator();

              ui.label("Flags:");
              let flags = pixel.flags;
              ui.indent("flags_indent", |ui| {
                flag_label(ui, flags, PixelFlags::DIRTY, "DIRTY", "needs simulation");
                flag_label(
                  ui,
                  flags,
                  PixelFlags::SOLID,
                  "SOLID",
                  "solid/powder material",
                );
                flag_label(ui, flags, PixelFlags::FALLING, "FALLING", "has momentum");
                flag_label(ui, flags, PixelFlags::BURNING, "BURNING", "on fire");
                flag_label(ui, flags, PixelFlags::WET, "WET", "wet");
                flag_label(
                  ui,
                  flags,
                  PixelFlags::PIXEL_BODY,
                  "PIXEL_BODY",
                  "belongs to body",
                );
              });

              ui.separator();
              ui.label(format!("Raw flags: 0b{:08b}", flags.bits()));

              if let Some((x, y)) = brush.world_pos {
                if let Ok(world) = worlds.single() {
                  let heat = world.get_heat_at(WorldPos::new(x, y)).unwrap_or(0);
                  let color = if heat > 0 {
                    egui::Color32::from_rgb(255, (255 - heat) / 2, 0)
                  } else {
                    egui::Color32::GRAY
                  };
                  ui.colored_label(color, format!("Heat: {}", heat));
                }
              }
            } else {
              ui.label("(no pixel data)");
            }
          } else {
            ui.label("(cursor outside window)");
          }
        });

      // Visual Debug section (collapsed by default)
      egui::CollapsingHeader::new("Visual Debug")
        .default_open(false)
        .show(ui, |ui| {
          if visual_debug_checkboxes(ui, &mut settings) {
            persistence.mark_changed();
          }
        });

      // Bodies debug section
      egui::CollapsingHeader::new("Bodies")
        .default_open(true)
        .show(ui, |ui| {
          let mut bodies: Vec<_> = body_query.iter().collect();
          if bodies.is_empty() {
            ui.label("No pixel bodies");
          } else {
            bodies.sort_by_key(|e| e.index());
            for entity in &bodies {
              let (status, color) = if let Ok(state) = submersion_query.get(*entity) {
                if state.is_submerged {
                  ("submerged", egui::Color32::LIGHT_BLUE)
                } else {
                  ("not submerged", egui::Color32::GRAY)
                }
              } else {
                ("no state", egui::Color32::DARK_GRAY)
              };
              ui.colored_label(color, format!("Body {}: {}", entity.index(), status));
            }
          }
        });
    });

  ui_state.pointer_over_ui = ctx.is_pointer_over_area();
}

/// Helper to display a flag with its status and description.
fn flag_label(ui: &mut egui::Ui, flags: PixelFlags, flag: PixelFlags, name: &str, desc: &str) {
  let set = flags.contains(flag);
  let status = if set { "[X]" } else { "[ ]" };
  let color = if set {
    egui::Color32::LIGHT_GREEN
  } else {
    egui::Color32::GRAY
  };
  ui.colored_label(color, format!("{} {} - {}", status, name, desc));
}

// ─── PhysicsPlugin ───────────────────────────────────────────────────────────

struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
  fn build(&self, _app: &mut App) {
    #[cfg(feature = "avian2d")]
    {
      _app.add_plugins(avian2d::prelude::PhysicsPlugins::default());
      _app.insert_resource(avian2d::prelude::Gravity(Vec2::new(0.0, -500.0)));
    }

    #[cfg(feature = "rapier2d")]
    {
      _app.add_plugins(
        bevy_rapier2d::prelude::RapierPhysicsPlugin::<bevy_rapier2d::prelude::NoUserData>::default(
        )
        .with_length_unit(50.0),
      );
    }

    #[cfg(any(feature = "avian2d", feature = "rapier2d"))]
    _app.add_systems(Update, (spawn_pixel_body, tag_new_bodies_as_bombs));
  }
}

/// Tags newly spawned pixel bodies as bombs.
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
fn tag_new_bodies_as_bombs(mut commands: Commands, new_bodies: Query<Entity, Added<PixelBody>>) {
  for entity in &new_bodies {
    commands.entity(entity).insert(Bomb {
      shell_depth: 0,
      blast_radius: 120.0,
      blast_strength: 60.0,
      detonated: false,
    });
  }
}

/// Spawns a random pixel body at the cursor when Space is pressed.
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
fn spawn_pixel_body(brush: Res<BrushState>, ui_state: Res<UiPointerState>, mut commands: Commands) {
  if !brush.spawn_requested || ui_state.pointer_over_ui {
    return;
  }

  let Some(pos) = brush.world_pos_f32 else {
    return;
  };

  let mut rng = rand::thread_rng();
  let sprite = if rng.gen_bool(0.5) {
    "box.png"
  } else {
    "femur.png"
  };

  commands.queue(SpawnPixelBody::new(sprite, material_ids::WOOD, pos));
}
