use std::path::PathBuf;
use std::time::Duration;

use bevy::camera::ScalingMode;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
// WASM compat: std::time::Instant panics on wasm32
use web_time::Instant;

use crate::StreamingCamera;

pub const CAMERA_SPEED: f32 = 500.0;
pub const SPEED_BOOST: f32 = 5.0;
pub const CAMERA_SETTINGS_FILE: &str = "camera_position.toml";
pub const CAMERA_DEBOUNCE_DURATION: Duration = Duration::from_millis(500);

pub struct PixelDebugControllerCameraPlugin;

impl Plugin for PixelDebugControllerCameraPlugin {
  fn build(&self, app: &mut App) {
    app
      .init_resource::<CameraZoom>()
      .add_systems(
        Startup,
        (load_camera_position, setup_camera, apply_camera_position).chain(),
      )
      .add_systems(
        Update,
        (
          camera_input,
          apply_camera_zoom,
          track_camera_changes,
          save_camera_position,
        )
          .chain(),
      );
  }
}

#[derive(Resource)]
pub struct CameraZoom {
  pub width: f32,
  pub height: f32,
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
  pub const PRESETS: &[(f32, f32, &'static str)] = &[
    (160.0, 120.0, "160x120"),
    (320.0, 240.0, "320x240"),
    (640.0, 480.0, "640x480"),
    (800.0, 600.0, "800x600"),
    (1280.0, 720.0, "1280x720"),
    (1920.0, 1080.0, "1920x1080"),
  ];

  pub fn zoom_in(&mut self) {
    self.width = (self.width * 0.8).max(80.0);
    self.height = (self.height * 0.8).max(60.0);
  }

  pub fn zoom_out(&mut self) {
    self.width = (self.width * 1.25).min(3840.0);
    self.height = (self.height * 1.25).min(2160.0);
  }
}

#[derive(Serialize, Deserialize, Default, Clone, Copy)]
pub struct CameraPosition {
  pub x: f32,
  pub y: f32,
}

#[derive(Resource)]
pub struct CameraPersistence {
  pub last_change: Option<Instant>,
  pub save_pending: bool,
  pub settings_path: Option<PathBuf>,
  pub last_position: CameraPosition,
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

pub fn get_camera_settings_path() -> Option<PathBuf> {
  #[cfg(feature = "native")]
  {
    let data_dir = dirs::data_dir()?;
    let app_dir = data_dir.join("bevy_pixel_world");
    Some(app_dir.join(CAMERA_SETTINGS_FILE))
  }
  #[cfg(not(feature = "native"))]
  {
    None
  }
}

fn setup_camera(mut commands: Commands) {
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
}

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

fn apply_camera_position(
  persistence: Res<CameraPersistence>,
  mut camera: Query<&mut Transform, With<StreamingCamera>>,
) {
  if let Ok(mut transform) = camera.single_mut() {
    transform.translation.x = persistence.last_position.x;
    transform.translation.y = persistence.last_position.y;
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
