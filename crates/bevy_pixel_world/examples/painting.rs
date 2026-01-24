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
//! - Side panel: Material selection, brush size slider
//!
//! Run with: `cargo run -p bevy_pixel_world --example painting`

use bevy::camera::ScalingMode;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};
#[cfg(feature = "diagnostics")]
use bevy_pixel_world::diagnostics::DiagnosticsPlugin;
use bevy_pixel_world::{
  material_ids, collision::CollisionQueryPoint, ColorIndex, MaterialId, MaterialSeeder, Materials,
  Pixel, PixelWorld, PixelWorldPlugin, SpawnPixelWorld, StreamingCamera, WorldRect,
};

const CAMERA_SPEED: f32 = 500.0;
const SPEED_BOOST: f32 = 5.0;
const MIN_RADIUS: u32 = 2;
const MAX_RADIUS: u32 = 100;
const DEFAULT_RADIUS: u32 = 15;

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
    // Enable persistence - paintings are saved automatically
    .add_plugins(PixelWorldPlugin::with_persistence("pixel_world_painting"))
    .add_plugins(EguiPlugin::default())
    .insert_resource(BrushState::default())
    .add_systems(Startup, setup)
    .add_systems(EguiPrimaryContextPass, ui_system)
    .add_systems(
      Update,
      (input_system, camera_input, paint_system, update_collision_query_point).chain(),
    );

  #[cfg(feature = "diagnostics")]
  app.add_plugins(DiagnosticsPlugin);

  app.run();
}

#[derive(Resource)]
struct BrushState {
  radius: u32,
  painting: bool,
  erasing: bool,
  world_pos: Option<(i64, i64)>,
  material: MaterialId,
}

impl Default for BrushState {
  fn default() -> Self {
    Self {
      radius: DEFAULT_RADIUS,
      painting: false,
      erasing: false,
      world_pos: None,
      material: material_ids::SAND,
    }
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

fn ui_system(mut contexts: EguiContexts, mut brush: ResMut<BrushState>, materials: Res<Materials>) {
  let Ok(ctx) = contexts.ctx_mut() else {
    return;
  };
  egui::SidePanel::left("brush_panel")
    .resizable(false)
    .show(ctx, |ui| {
      ui.heading("Brush");
      ui.separator();

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
      ui.add(egui::Slider::new(&mut radius, MIN_RADIUS as f32..=MAX_RADIUS as f32).text("Size"));
      brush.radius = radius as u32;
    });
}

fn input_system(
  mut brush: ResMut<BrushState>,
  mouse_buttons: Res<ButtonInput<MouseButton>>,
  mut scroll_events: MessageReader<MouseWheel>,
  window_query: Query<&Window, With<PrimaryWindow>>,
  camera_query: Query<(&Camera, &GlobalTransform), With<StreamingCamera>>,
) {
  brush.painting = mouse_buttons.pressed(MouseButton::Left);
  brush.erasing = mouse_buttons.pressed(MouseButton::Right);

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
    }
  } else {
    brush.world_pos = None;
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

fn paint_system(
  brush: Res<BrushState>,
  mut worlds: Query<&mut PixelWorld>,
  gizmos: bevy_pixel_world::debug_shim::GizmosParam,
) {
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
