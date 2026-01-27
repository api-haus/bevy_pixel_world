use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::collision::CollisionQueryPoint;
use crate::{MaterialId, StreamingCamera, material_ids};

pub const MIN_RADIUS: u32 = 2;
pub const MAX_RADIUS: u32 = 100;
pub const DEFAULT_RADIUS: u32 = 15;

pub struct PixelDebugControllerPlugin;

impl Plugin for PixelDebugControllerPlugin {
  fn build(&self, app: &mut App) {
    app
      .insert_resource(BrushState::default())
      .add_systems(Startup, spawn_collision_query_point)
      .add_systems(
        Update,
        (
          input_system,
          paint_system.after(input_system),
          update_collision_query_point.after(input_system),
        ),
      );
  }
}

#[derive(Resource)]
pub struct BrushState {
  pub radius: u32,
  pub painting: bool,
  pub erasing: bool,
  pub world_pos: Option<(i64, i64)>,
  pub world_pos_f32: Option<Vec2>,
  pub material: MaterialId,
  pub spawn_requested: bool,
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

fn spawn_collision_query_point(mut commands: Commands) {
  commands.spawn((Transform::default(), CollisionQueryPoint));
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

  for event in scroll_events.read() {
    let delta = match event.unit {
      MouseScrollUnit::Line => event.y as i32 * 3,
      MouseScrollUnit::Pixel => (event.y / 10.0) as i32,
    };
    let new_radius = (brush.radius as i32 + delta).clamp(MIN_RADIUS as i32, MAX_RADIUS as i32);
    brush.radius = new_radius as u32;
  }

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

fn paint_system(
  brush: Res<BrushState>,
  ui_over: Option<Res<UiPointerState>>,
  mut worlds: Query<&mut crate::PixelWorld>,
  gizmos: crate::debug_shim::GizmosParam,
) {
  if ui_over.is_some_and(|s| s.pointer_over_ui) {
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

  let (material, color) = if brush.erasing {
    (material_ids::VOID, crate::ColorIndex(0))
  } else {
    (brush.material, crate::ColorIndex(128))
  };
  let brush_pixel = crate::Pixel::new(material, color);

  let radius = brush.radius;
  let radius_i64 = radius as i64;
  let radius_sq = (radius_i64 * radius_i64) as f32;

  let rect = crate::WorldRect::centered(center_x, center_y, radius);

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
  if let Some((x, y)) = brush.world_pos
    && let Ok(mut transform) = query_points.single_mut()
  {
    transform.translation = Vec3::new(x as f32, y as f32, 0.0);
  }
}

/// Resource that external UI plugins can insert to signal the pointer is over
/// UI. If not present, painting always proceeds.
#[derive(Resource, Default)]
pub struct UiPointerState {
  pub pointer_over_ui: bool,
}
