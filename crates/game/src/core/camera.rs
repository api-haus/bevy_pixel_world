use bevy::{camera::ScalingMode, prelude::*};

use crate::config::ConfigLoaded;

/// Marker component for the game camera
#[derive(Component)]
pub struct GameCamera;

/// Marker component for entities the camera should follow
#[derive(Component)]
pub struct CameraTarget;

/// Camera smoothing factor (higher = snappier, lower = smoother)
#[derive(Resource)]
pub struct CameraSmoothness(pub f32);

impl Default for CameraSmoothness {
  fn default() -> Self {
    Self(8.0)
  }
}

/// Simple orthographic 2D camera setup
pub fn setup_camera(mut commands: Commands, config: Res<ConfigLoaded>) {
  commands.spawn((
    GameCamera,
    Camera2d,
    Camera {
      order: 0,
      clear_color: ClearColorConfig::Custom(Color::BLACK),
      ..default()
    },
    Projection::Orthographic(OrthographicProjection {
      near: -1000.0,
      far: 1000.0,
      scale: 1.0,
      viewport_origin: Vec2::new(0.5, 0.5),
      scaling_mode: ScalingMode::AutoMin {
        min_width: config.camera.viewport_width,
        min_height: config.camera.viewport_height,
      },
      area: Rect::default(),
    }),
  ));
}

/// Camera follow with dampening
pub fn camera_follow(
  target_query: Query<&GlobalTransform, (With<CameraTarget>, Without<GameCamera>)>,
  mut camera_query: Query<&mut Transform, With<GameCamera>>,
  smoothness: Res<CameraSmoothness>,
  time: Res<Time>,
) {
  let Ok(target) = target_query.single() else {
    return;
  };
  let Ok(mut camera_transform) = camera_query.single_mut() else {
    return;
  };

  let target_pos = target.translation();
  let current_pos = camera_transform.translation;

  // Exponential smoothing: lerp factor based on time and smoothness
  let t = (smoothness.0 * time.delta_secs()).min(1.0);

  camera_transform.translation.x = current_pos.x.lerp(target_pos.x, t);
  camera_transform.translation.y = current_pos.y.lerp(target_pos.y, t);
}
