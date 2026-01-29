use bevy::{camera::ScalingMode, prelude::*};
use bevy_pixel_world::{PixelCamera, StreamingCamera, VirtualCamera};

use crate::config::ConfigLoaded;

/// Marker component for the game camera
#[derive(Component)]
pub struct GameCamera;

/// Marker component for the player's virtual camera
#[derive(Component)]
pub struct PlayerVirtualCamera;

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
  // Spawn the real camera for rendering
  commands.spawn((
    GameCamera,
    StreamingCamera,        // Required for PixelWorld chunk streaming
    PixelCamera::default(), // Enable pixel-perfect rendering
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

  // Spawn a virtual camera at player priority (lowest, default)
  commands.spawn((
    PlayerVirtualCamera,
    VirtualCamera::new(VirtualCamera::PRIORITY_PLAYER),
    Transform::default(),
  ));
}

/// Camera follow with dampening - updates the virtual camera's transform
pub fn camera_follow(
  target_query: Query<&GlobalTransform, (With<CameraTarget>, Without<PlayerVirtualCamera>)>,
  mut camera_query: Query<&mut Transform, With<PlayerVirtualCamera>>,
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
