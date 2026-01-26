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

use std::path::PathBuf;
use std::time::{Duration, Instant};

use bevy::camera::ScalingMode;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::time::{Timer, TimerMode};
use bevy::window::PrimaryWindow;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
use bevy_pixel_world::SpawnPixelBody;
#[cfg(feature = "diagnostics")]
use bevy_pixel_world::diagnostics::DiagnosticsPlugin;
#[cfg(feature = "submergence")]
use bevy_pixel_world::submergence::{PixelSubmergencePlugin, SubmersionState};
#[cfg(feature = "visual_debug")]
use bevy_pixel_world::visual_debug::{
  SettingsPersistence, VisualDebugSettings, visual_debug_checkboxes,
};
use bevy_pixel_world::{
  ColorIndex, MaterialId, MaterialSeeder, Materials, PersistenceControl, Pixel, PixelFlags,
  PixelWorld, PixelWorldPlugin, SpawnPixelWorld, StreamingCamera, WorldPos, WorldRect,
  collision::CollisionQueryPoint, material_ids,
};
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
use rand::Rng;
use serde::{Deserialize, Serialize};

const CAMERA_SPEED: f32 = 500.0;
const SPEED_BOOST: f32 = 5.0;
const MIN_RADIUS: u32 = 2;
const MAX_RADIUS: u32 = 100;
const DEFAULT_RADIUS: u32 = 15;

const CAMERA_SETTINGS_FILE: &str = "camera_position.toml";
const CAMERA_DEBOUNCE_DURATION: Duration = Duration::from_millis(500);

/// Serializable camera position.
#[derive(Serialize, Deserialize, Default, Clone, Copy)]
struct CameraPosition {
  x: f32,
  y: f32,
}

/// Tracks camera position changes for debounced saving.
#[derive(Resource)]
struct CameraPersistence {
  last_change: Option<Instant>,
  save_pending: bool,
  settings_path: Option<PathBuf>,
  last_position: CameraPosition,
}

impl Default for CameraPersistence {
  fn default() -> Self {
    Self {
      last_change: None,
      save_pending: false,
      settings_path: get_camera_settings_path(),
      last_position: CameraPosition::default(),
    }
  }
}

fn get_camera_settings_path() -> Option<PathBuf> {
  let data_dir = dirs::data_dir()?;
  let app_dir = data_dir.join("bevy_pixel_world");
  Some(app_dir.join(CAMERA_SETTINGS_FILE))
}

fn main() {
  let mut app = App::new();

  app
    .add_plugins(DefaultPlugins.set(WindowPlugin {
      primary_window: Some(Window {
        title: "Brush Painting Demo - PixelWorld".to_string(),
        resolution: (1280, 720).into(),
        ..default()
      }),
      ..default()
    }))
    // Enable persistence with named save "world"
    .add_plugins(PixelWorldPlugin::with_persistence("pixel_world_painting").load("world"))
    .add_plugins(EguiPlugin::default())
    .insert_resource(BrushState::default())
    .init_resource::<UiState>()
    .init_resource::<CameraZoom>()
    .add_systems(
      Startup,
      (load_camera_position, setup, apply_camera_position).chain(),
    )
    .add_systems(EguiPrimaryContextPass, ui_system)
    .add_systems(
      Update,
      (
        input_system,
        camera_input,
        apply_camera_zoom,
        track_camera_changes,
        save_camera_position,
        paint_system,
        update_collision_query_point,
        handle_save_hotkey,
        auto_save_system,
      )
        .chain(),
    );

  #[cfg(feature = "diagnostics")]
  app.add_plugins(DiagnosticsPlugin);

  #[cfg(feature = "avian2d")]
  {
    app.add_plugins(avian2d::prelude::PhysicsPlugins::default());
    // Scale gravity for pixel coordinates (default is 9.81 m/s², we need ~500
    // px/s²)
    app.insert_resource(avian2d::prelude::Gravity(Vec2::new(0.0, -500.0)));
  }

  #[cfg(feature = "rapier2d")]
  {
    // Use length_unit to scale gravity for pixel coordinates (9.81 * 50 ≈ 490
    // px/s²)
    app.add_plugins(
      bevy_rapier2d::prelude::RapierPhysicsPlugin::<bevy_rapier2d::prelude::NoUserData>::default()
        .with_length_unit(50.0),
    );
  }

  #[cfg(any(feature = "avian2d", feature = "rapier2d"))]
  app.add_systems(Update, spawn_pixel_body.after(input_system));

  // Enable submergence detection - all pixel bodies will automatically
  // have their gravity and damping adjusted based on liquid submersion
  #[cfg(feature = "submergence")]
  app.add_plugins(PixelSubmergencePlugin::default());

  app.run();
}

#[derive(Resource)]
struct BrushState {
  radius: u32,
  painting: bool,
  erasing: bool,
  world_pos: Option<(i64, i64)>,
  world_pos_f32: Option<Vec2>,
  material: MaterialId,
  spawn_requested: bool,
}

impl Default for BrushState {
  fn default() -> Self {
    Self {
      radius: DEFAULT_RADIUS,
      painting: false,
      erasing: false,
      world_pos: None,
      world_pos_f32: None,
      material: material_ids::SAND,
      spawn_requested: false,
    }
  }
}

/// Tracks whether the pointer is over UI elements.
#[derive(Resource, Default)]
struct UiState {
  pointer_over_ui: bool,
}

/// Camera zoom/viewport settings.
#[derive(Resource)]
struct CameraZoom {
  width: f32,
  height: f32,
}

impl Default for CameraZoom {
  fn default() -> Self {
    Self {
      width: 640.0,
      height: 480.0,
    }
  }
}

impl CameraZoom {
  /// Preset resolutions (width, height, label).
  const PRESETS: &[(f32, f32, &'static str)] = &[
    (160.0, 120.0, "160x120"),
    (320.0, 240.0, "320x240"),
    (640.0, 480.0, "640x480"),
    (800.0, 600.0, "800x600"),
    (1280.0, 720.0, "1280x720"),
    (1920.0, 1080.0, "1920x1080"),
  ];

  /// Zoom in (smaller viewport = more zoomed in).
  fn zoom_in(&mut self) {
    self.width = (self.width * 0.8).max(80.0);
    self.height = (self.height * 0.8).max(60.0);
  }

  /// Zoom out (larger viewport = more zoomed out).
  fn zoom_out(&mut self) {
    self.width = (self.width * 1.25).min(3840.0);
    self.height = (self.height * 1.25).min(2160.0);
  }
}

fn setup(mut commands: Commands) {
  // Spawn camera with StreamingCamera marker
  commands.spawn((
    Camera2d,
    StreamingCamera,
    Projection::Orthographic(OrthographicProjection {
      near: -1000.0,
      far: 1000.0,
      scale: 1.0,
      viewport_origin: Vec2::new(0.5, 0.5),
      scaling_mode: ScalingMode::AutoMin {
        min_width: 640.0,
        min_height: 480.0,
      },
      area: Rect::default(),
    }),
  ));

  // Spawn the pixel world (Materials and mesh are handled by the plugin)
  commands.queue(SpawnPixelWorld::new(MaterialSeeder::new(42)));

  // Spawn collision query point that follows the mouse cursor
  commands.spawn((Transform::default(), CollisionQueryPoint));
}

#[allow(unused_mut, unused_variables)]
fn ui_system(
  mut contexts: EguiContexts,
  mut brush: ResMut<BrushState>,
  mut zoom: ResMut<CameraZoom>,
  materials: Res<Materials>,
  mut ui_state: ResMut<UiState>,
  worlds: Query<&PixelWorld>,
  #[cfg(feature = "visual_debug")] mut settings: ResMut<VisualDebugSettings>,
  #[cfg(feature = "visual_debug")] mut persistence: ResMut<SettingsPersistence>,
  #[cfg(feature = "submergence")] submersion_query: Query<(Entity, &SubmersionState)>,
) {
  let Ok(ctx) = contexts.ctx_mut() else {
    return;
  };

  // Get pixel at cursor for debug display
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
          // Material picker (skip VOID)
          for id in [
            material_ids::SOIL,
            material_ids::STONE,
            material_ids::SAND,
            material_ids::WATER,
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

          // Brush size slider
          let mut radius = brush.radius as f32;
          ui.add(
            egui::Slider::new(&mut radius, MIN_RADIUS as f32..=MAX_RADIUS as f32).text("Size"),
          );
          brush.radius = radius as u32;
        });

      // Camera section
      egui::CollapsingHeader::new("Camera")
        .default_open(true)
        .show(ui, |ui| {
          // Current resolution display
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

          // Preset buttons in rows of 2
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

              // Material
              let mat_name = if pixel.material.0 == 0 {
                "VOID"
              } else {
                materials.get(pixel.material).name
              };
              ui.label(format!("Material: {} ({})", mat_name, pixel.material.0));

              // Color
              ui.label(format!("Color: {}", pixel.color.0));

              // Damage
              ui.label(format!("Damage: {}", pixel.damage));

              ui.separator();

              // Flags with disambiguation
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
            } else {
              ui.label("(no pixel data)");
            }
          } else {
            ui.label("(cursor outside window)");
          }
        });

      // Visual Debug section (feature-gated, collapsed by default)
      #[cfg(feature = "visual_debug")]
      egui::CollapsingHeader::new("Visual Debug")
        .default_open(false)
        .show(ui, |ui| {
          if visual_debug_checkboxes(ui, &mut settings) {
            persistence.mark_changed();
          }
        });

      // Submergence section (feature-gated)
      #[cfg(feature = "submergence")]
      egui::CollapsingHeader::new("Submergence")
        .default_open(true)
        .show(ui, |ui| {
          let mut bodies: Vec<_> = submersion_query.iter().collect();
          if bodies.is_empty() {
            ui.label("No pixel bodies");
          } else {
            bodies.sort_by_key(|(e, _)| e.index());
            for (entity, state) in bodies.iter().take(5) {
              let status = if state.is_submerged {
                "SUBMERGED"
              } else {
                "surface"
              };
              let color = if state.is_submerged {
                egui::Color32::LIGHT_BLUE
              } else {
                egui::Color32::GRAY
              };
              ui.colored_label(
                color,
                format!(
                  "Body {}: {} ({:.0}%)",
                  entity.index(),
                  status,
                  state.submerged_fraction * 100.0
                ),
              );
            }
            if bodies.len() > 5 {
              ui.label(format!("... and {} more", bodies.len() - 5));
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

fn input_system(
  mut brush: ResMut<BrushState>,
  mouse_buttons: Res<ButtonInput<MouseButton>>,
  keys: Res<ButtonInput<KeyCode>>,
  mut scroll_events: MessageReader<MouseWheel>,
  window_query: Query<&Window, With<PrimaryWindow>>,
  camera_query: Query<(&Camera, &GlobalTransform), With<StreamingCamera>>,
) {
  brush.painting = mouse_buttons.pressed(MouseButton::Left);
  brush.erasing = mouse_buttons.pressed(MouseButton::Right);
  brush.spawn_requested = keys.just_pressed(KeyCode::Space);

  // Handle scroll wheel for radius
  for event in scroll_events.read() {
    let delta = match event.unit {
      MouseScrollUnit::Line => event.y as i32 * 3,
      MouseScrollUnit::Pixel => (event.y / 10.0) as i32,
    };
    let new_radius = (brush.radius as i32 + delta).clamp(MIN_RADIUS as i32, MAX_RADIUS as i32);
    brush.radius = new_radius as u32;
  }

  // Convert mouse position to world coordinates
  let Ok(window) = window_query.single() else {
    return;
  };
  let Ok((camera, camera_transform)) = camera_query.single() else {
    return;
  };

  if let Some(cursor_pos) = window.cursor_position() {
    if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
      brush.world_pos = Some((world_pos.x as i64, world_pos.y as i64));
      brush.world_pos_f32 = Some(world_pos);
    }
  } else {
    brush.world_pos = None;
    brush.world_pos_f32 = None;
  }
}

fn camera_input(
  keys: Res<ButtonInput<KeyCode>>,
  mut camera: Query<&mut Transform, With<StreamingCamera>>,
  time: Res<Time>,
) {
  let mut direction = Vec2::ZERO;

  if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
    direction.y += 1.0;
  }
  if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
    direction.y -= 1.0;
  }
  if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
    direction.x -= 1.0;
  }
  if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
    direction.x += 1.0;
  }

  if direction == Vec2::ZERO {
    return;
  }

  let direction = direction.normalize();
  let speed = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
    CAMERA_SPEED * SPEED_BOOST
  } else {
    CAMERA_SPEED
  };

  if let Ok(mut transform) = camera.single_mut() {
    transform.translation.x += direction.x * speed * time.delta_secs();
    transform.translation.y += direction.y * speed * time.delta_secs();
  }
}

/// Applies camera zoom settings to the orthographic projection.
fn apply_camera_zoom(
  zoom: Res<CameraZoom>,
  mut camera: Query<&mut Projection, With<StreamingCamera>>,
) {
  if !zoom.is_changed() {
    return;
  }

  let Ok(mut projection) = camera.single_mut() else {
    return;
  };

  if let Projection::Orthographic(ref mut ortho) = *projection {
    ortho.scaling_mode = ScalingMode::AutoMin {
      min_width: zoom.width,
      min_height: zoom.height,
    };
  }
}

fn paint_system(
  brush: Res<BrushState>,
  ui_state: Res<UiState>,
  mut worlds: Query<&mut PixelWorld>,
  gizmos: bevy_pixel_world::debug_shim::GizmosParam,
) {
  // Don't paint when pointer is over UI
  if ui_state.pointer_over_ui {
    return;
  }

  if !brush.painting && !brush.erasing {
    return;
  }

  let Some((center_x, center_y)) = brush.world_pos else {
    return;
  };

  let Ok(mut world) = worlds.single_mut() else {
    return;
  };

  // Use selected material for painting, VOID for erasing
  let (material, color) = if brush.erasing {
    (material_ids::VOID, ColorIndex(0))
  } else {
    (brush.material, ColorIndex(128))
  };
  let brush_pixel = Pixel::new(material, color);

  let radius = brush.radius;
  let radius_i64 = radius as i64;
  let radius_sq = (radius_i64 * radius_i64) as f32;

  // Use the blit API for parallel painting
  let rect = WorldRect::centered(center_x, center_y, radius);

  world.blit(
    rect,
    |frag| {
      let dx = frag.x - center_x;
      let dy = frag.y - center_y;
      let dist_sq = (dx * dx + dy * dy) as f32;

      if dist_sq <= radius_sq {
        Some(brush_pixel)
      } else {
        None
      }
    },
    gizmos.get(),
  );
}

fn update_collision_query_point(
  brush: Res<BrushState>,
  mut query_points: Query<&mut Transform, With<CollisionQueryPoint>>,
) {
  if let Some((x, y)) = brush.world_pos {
    if let Ok(mut transform) = query_points.single_mut() {
      transform.translation = Vec3::new(x as f32, y as f32, 0.0);
    }
  }
}

/// Spawns a random pixel body at the cursor when Space is pressed.
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
fn spawn_pixel_body(brush: Res<BrushState>, ui_state: Res<UiState>, mut commands: Commands) {
  if !brush.spawn_requested || ui_state.pointer_over_ui {
    return;
  }

  let Some(pos) = brush.world_pos_f32 else {
    return;
  };

  // Randomly choose between box.png and femur.png
  let mut rng = rand::thread_rng();
  let sprite = if rng.gen_bool(0.5) {
    "box.png"
  } else {
    "femur.png"
  };

  commands.queue(SpawnPixelBody::new(sprite, material_ids::WOOD, pos));
}

/// Loads camera position from disk on startup.
fn load_camera_position(mut commands: Commands) {
  let position = match get_camera_settings_path() {
    Some(path) if path.exists() => match std::fs::read_to_string(&path) {
      Ok(contents) => match toml::from_str(&contents) {
        Ok(pos) => {
          info!("Loaded camera position from {}", path.display());
          pos
        }
        Err(e) => {
          warn!("Failed to parse camera position: {e}, using default");
          CameraPosition::default()
        }
      },
      Err(e) => {
        warn!("Failed to read camera position: {e}, using default");
        CameraPosition::default()
      }
    },
    _ => CameraPosition::default(),
  };

  commands.insert_resource(CameraPersistence {
    last_position: position,
    ..default()
  });
}

/// Applies loaded camera position to the camera transform.
fn apply_camera_position(
  persistence: Res<CameraPersistence>,
  mut camera: Query<&mut Transform, With<StreamingCamera>>,
) {
  if let Ok(mut transform) = camera.single_mut() {
    transform.translation.x = persistence.last_position.x;
    transform.translation.y = persistence.last_position.y;
  }
}

/// Tracks camera movement and marks persistence as changed.
fn track_camera_changes(
  camera: Query<&Transform, With<StreamingCamera>>,
  mut persistence: ResMut<CameraPersistence>,
) {
  let Ok(transform) = camera.single() else {
    return;
  };

  let current = CameraPosition {
    x: transform.translation.x,
    y: transform.translation.y,
  };

  if (current.x - persistence.last_position.x).abs() > 0.01
    || (current.y - persistence.last_position.y).abs() > 0.01
  {
    persistence.last_position = current;
    persistence.last_change = Some(Instant::now());
    persistence.save_pending = true;
  }
}

/// Saves camera position to disk when changed (debounced).
fn save_camera_position(mut persistence: ResMut<CameraPersistence>) {
  if !persistence.save_pending {
    return;
  }

  let Some(last_change) = persistence.last_change else {
    return;
  };

  if last_change.elapsed() < CAMERA_DEBOUNCE_DURATION {
    return;
  }

  persistence.save_pending = false;

  let Some(path) = &persistence.settings_path else {
    return;
  };

  if let Some(parent) = path.parent()
    && let Err(e) = std::fs::create_dir_all(parent)
  {
    warn!("Failed to create settings directory: {e}");
    return;
  }

  match toml::to_string_pretty(&persistence.last_position) {
    Ok(contents) => {
      if let Err(e) = std::fs::write(path, contents) {
        warn!("Failed to write camera position: {e}");
      } else {
        debug!("Saved camera position to {}", path.display());
      }
    }
    Err(e) => {
      warn!("Failed to serialize camera position: {e}");
    }
  }
}

/// Handles Ctrl+S to trigger manual save.
fn handle_save_hotkey(
  keys: Res<ButtonInput<KeyCode>>,
  mut persistence: ResMut<PersistenceControl>,
) {
  let ctrl_pressed = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
  let s_pressed = keys.just_pressed(KeyCode::KeyS);

  if ctrl_pressed && s_pressed {
    let handle = persistence.save("world");
    info!("Manual save requested (id: {})", handle.id());
  }
}

/// Auto-save system that saves periodically.
fn auto_save_system(
  time: Res<Time>,
  mut timer: Local<Option<Timer>>,
  mut persistence: ResMut<PersistenceControl>,
) {
  let timer = timer.get_or_insert_with(|| Timer::from_seconds(5.0, TimerMode::Repeating));
  timer.tick(time.delta());

  if timer.just_finished() {
    persistence.save("world");
  }
}
