use bevy::{camera::ScalingMode, prelude::*};
use bevy_pixel_world::{PixelCamera, StreamingCamera, VirtualCamera};

use crate::config::ConfigLoaded;
use crate::player::components::{Player, VisualPosition};

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

/// Camera follow - updates the virtual camera's transform to match player's
/// interpolated position. Uses the EXACT same VisualPosition as the sprite to
/// prevent jitter.
pub fn camera_follow(
  player_query: Query<&VisualPosition, With<Player>>,
  mut camera_query: Query<&mut Transform, With<PlayerVirtualCamera>>,
) {
  let Ok(visual_pos) = player_query.single() else {
    return;
  };
  let Ok(mut camera_transform) = camera_query.single_mut() else {
    return;
  };

  // Use exact same position as sprite - no smoothing!
  // Pixel camera will handle snapping uniformly for both.
  camera_transform.translation.x = visual_pos.0.x;
  camera_transform.translation.y = visual_pos.0.y;
}
