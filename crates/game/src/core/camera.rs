use bevy::{camera::ScalingMode, prelude::*};

use crate::config::ConfigLoaded;
use crate::pixel_world::{PixelCamera, StreamingCamera};
use crate::player::components::{Player, VisualPosition};

/// Marker component for the game camera
#[derive(Component)]
pub struct GameCamera;

/// Simple orthographic 2D camera setup with pixel-perfect rendering.
pub fn setup_camera(mut commands: Commands, config: Res<ConfigLoaded>) {
  commands.spawn((
    GameCamera,
    StreamingCamera,        // Required for PixelWorld chunk streaming
    PixelCamera::default(), // Pixel-perfect subpixel compensation
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

/// Camera follows the interpolated visual position of the player.
pub fn camera_follow(
  player_query: Query<&VisualPosition, With<Player>>,
  mut camera_query: Query<&mut Transform, (With<GameCamera>, Without<Player>)>,
) {
  let Ok(visual_pos) = player_query.single() else {
    return;
  };
  let Ok(mut camera_tf) = camera_query.single_mut() else {
    return;
  };

  camera_tf.translation.x = visual_pos.0.x;
  camera_tf.translation.y = visual_pos.0.y;
}
