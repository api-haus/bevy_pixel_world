use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::pixel_world::collision::CollisionQueryPoint;
use crate::pixel_world::pixel_camera::LogicalCameraPosition;
use crate::pixel_world::{MaterialId, StreamingCamera, material_ids};

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
          heat_paint_system.after(input_system),
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
  /// When true, LMB paints heat values instead of materials.
  pub heat_painting: bool,
  /// Heat value to paint (0-255).
  pub heat_value: u8,
  /// When false, brush painting is disabled (e.g., in level editor mode).
  pub enabled: bool,
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
      heat_painting: false,
      heat_value: 100,
      enabled: true,
    }
  }
}

fn spawn_collision_query_point(mut commands: Commands) {
  commands.spawn((Transform::default(), CollisionQueryPoint));
}

fn input_system(
  mut brush: ResMut<BrushState>,
  mouse_buttons: Res<ButtonInput<MouseButton>>,
  mut scroll_events: MessageReader<MouseWheel>,
  window_query: Query<&Window, With<PrimaryWindow>>,
  camera_query: Query<
    (
      &Camera,
      &GlobalTransform,
      &Projection,
      Option<&LogicalCameraPosition>,
    ),
    With<StreamingCamera>,
  >,
) {
  brush.painting = mouse_buttons.pressed(MouseButton::Left);
  brush.erasing = mouse_buttons.pressed(MouseButton::Right);

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
  let Ok((camera, camera_transform, projection, logical_pos)) = camera_query.single() else {
    return;
  };

  if let Some(cursor_pos) = window.cursor_position() {
    // With pixel camera, the scene camera renders to a texture, so
    // viewport_to_world_2d doesn't work correctly. Compute world position
    // manually from window coordinates.
    let world_pos = if let Some(logical) = logical_pos {
      // Pixel camera mode: compute manually using logical position
      let Projection::Orthographic(ortho) = projection else {
        return;
      };

      // Get orthographic view half-size
      let half_width = (ortho.area.max.x - ortho.area.min.x) / 2.0;
      let half_height = (ortho.area.max.y - ortho.area.min.y) / 2.0;

      if half_width <= 0.0 || half_height <= 0.0 {
        return; // Ortho area not computed yet
      }

      // Convert cursor to normalized coordinates (0 to 1)
      let normalized = cursor_pos / Vec2::new(window.width(), window.height());

      // Convert to clip space (-1 to 1), with Y flipped (screen Y down, world Y up)
      let clip = Vec2::new(
        (normalized.x - 0.5) * 2.0,
        (0.5 - normalized.y) * 2.0, // Flip Y
      );

      // Map to world offset from camera center
      let world_offset = Vec2::new(clip.x * half_width, clip.y * half_height);

      // Add logical camera position
      logical.0 + world_offset
    } else {
      // Normal mode: use standard viewport_to_world_2d
      let Ok(pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) else {
        return;
      };
      pos
    };

    brush.world_pos = Some((world_pos.x as i64, world_pos.y as i64));
    brush.world_pos_f32 = Some(world_pos);
  } else {
    brush.world_pos = None;
    brush.world_pos_f32 = None;
  }
}

fn paint_system(
  brush: Res<BrushState>,
  ui_over: Option<Res<UiPointerState>>,
  mut worlds: Query<&mut crate::pixel_world::PixelWorld>,
  gizmos: crate::pixel_world::debug_shim::GizmosParam,
) {
  if !brush.enabled {
    return;
  }
  if ui_over.is_some_and(|s| s.pointer_over_ui) {
    return;
  }

  // Skip material painting when in heat painting mode (heat paint system handles
  // it)
  if brush.heat_painting && brush.painting {
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
    (material_ids::VOID, crate::pixel_world::ColorIndex(0))
  } else {
    (brush.material, crate::pixel_world::ColorIndex(128))
  };
  let brush_pixel = crate::pixel_world::Pixel::new(material, color);

  let radius = brush.radius;
  let radius_i64 = radius as i64;
  let radius_sq = (radius_i64 * radius_i64) as f32;

  let rect = crate::pixel_world::WorldRect::centered(center_x, center_y, radius);

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

fn heat_paint_system(
  brush: Res<BrushState>,
  ui_over: Option<Res<UiPointerState>>,
  mut worlds: Query<&mut crate::pixel_world::PixelWorld>,
) {
  if !brush.enabled {
    return;
  }
  if !brush.heat_painting || !brush.painting {
    return;
  }
  if ui_over.is_some_and(|s| s.pointer_over_ui) {
    return;
  }
  let Some((center_x, center_y)) = brush.world_pos else {
    return;
  };
  let Ok(mut world) = worlds.single_mut() else {
    return;
  };

  let radius = brush.radius as i64;
  let radius_sq = (radius * radius) as f32;
  let heat = brush.heat_value;

  for dy in -radius..=radius {
    for dx in -radius..=radius {
      let dist_sq = (dx * dx + dy * dy) as f32;
      if dist_sq <= radius_sq {
        let pos = crate::pixel_world::WorldPos::new(center_x + dx, center_y + dy);
        world.set_heat_at(pos, heat);
      }
    }
  }
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
