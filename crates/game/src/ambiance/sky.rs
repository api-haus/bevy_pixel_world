use bevy::prelude::*;

use crate::config::{ConfigLoaded, SkyKeyframe};
use crate::core::camera::GameCamera;
use crate::time_of_day::TimeOfDay;

/// Marker component for the sky sprite.
#[derive(Component)]
pub struct SkySprite;

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
  fn build(&self, app: &mut App) {
    app
      .add_systems(Startup, spawn_sky)
      .add_systems(Update, update_sky_color)
      .add_systems(PostUpdate, sync_sky_position);
  }
}

fn spawn_sky(mut commands: Commands, config: Res<ConfigLoaded>) {
  // Make sky much larger than viewport to handle any window size/aspect ratio
  let size = (config
    .camera
    .viewport_width
    .max(config.camera.viewport_height)
    * 4.0)
    .max(4000.0);

  // Initial color from config at start hour
  let initial_color =
    interpolate_sky_color(config.day_cycle.start_hour, &config.day_cycle.sky_colors);

  info!(
    "Sky spawned: size={}, initial_color={:?}, start_hour={}",
    size, initial_color, config.day_cycle.start_hour
  );

  commands.spawn((
    SkySprite,
    Sprite {
      color: Color::srgb(initial_color[0], initial_color[1], initial_color[2]),
      custom_size: Some(Vec2::new(size, size)),
      ..default()
    },
    Transform::from_xyz(0.0, 0.0, -900.0),
  ));
}

fn update_sky_color(
  time_of_day: Res<TimeOfDay>,
  config: Res<ConfigLoaded>,
  mut sky_query: Query<&mut Sprite, With<SkySprite>>,
) {
  if !time_of_day.is_changed() && !config.is_changed() {
    return;
  }

  let color = interpolate_sky_color(time_of_day.hour, &config.day_cycle.sky_colors);

  for mut sprite in &mut sky_query {
    sprite.color = Color::srgb(color[0], color[1], color[2]);
    debug!(
      "Sky color updated: hour={:.1}, color={:?}",
      time_of_day.hour, color
    );
  }
}

fn sync_sky_position(
  camera_query: Query<&Transform, (With<GameCamera>, Without<SkySprite>)>,
  mut sky_query: Query<&mut Transform, With<SkySprite>>,
) {
  let Ok(camera_transform) = camera_query.single() else {
    return;
  };

  for mut transform in &mut sky_query {
    transform.translation.x = camera_transform.translation.x;
    transform.translation.y = camera_transform.translation.y;
    // Keep Z at -900
  }
}

fn interpolate_sky_color(hour: f32, keyframes: &[SkyKeyframe]) -> [f32; 3] {
  if keyframes.is_empty() {
    return [0.0, 0.0, 0.0];
  }

  if keyframes.len() == 1 {
    return keyframes[0].color;
  }

  // Find the two keyframes that bracket the current hour
  let mut prev_idx = 0;
  let mut next_idx = 0;

  for (i, kf) in keyframes.iter().enumerate() {
    if kf.hour <= hour {
      prev_idx = i;
    }
    if kf.hour > hour && next_idx == 0 {
      next_idx = i;
      break;
    }
  }

  // If we didn't find a next keyframe, wrap to the first one
  if next_idx == 0 {
    next_idx = 0;
    // Use the last keyframe as prev if hour is past all keyframes
    prev_idx = keyframes.len() - 1;
  }

  let prev = &keyframes[prev_idx];
  let next = &keyframes[next_idx];

  // Handle wrap-around case (prev is after next in the list)
  let (prev_hour, next_hour) = if prev.hour > next.hour {
    // Wrapping around midnight
    let effective_next_hour = next.hour + 24.0;
    let effective_hour = if hour < prev.hour { hour + 24.0 } else { hour };
    (
      prev.hour,
      if effective_hour >= prev.hour {
        effective_next_hour
      } else {
        next.hour
      },
    )
  } else {
    (prev.hour, next.hour)
  };

  // Calculate interpolation factor
  let range = next_hour - prev_hour;
  let t = if range > 0.0 {
    let effective_hour = if prev.hour > next.hour && hour < prev.hour {
      hour + 24.0
    } else {
      hour
    };
    ((effective_hour - prev_hour) / range).clamp(0.0, 1.0)
  } else {
    0.0
  };

  // Linear interpolation
  [
    prev.color[0] + t * (next.color[0] - prev.color[0]),
    prev.color[1] + t * (next.color[1] - prev.color[1]),
    prev.color[2] + t * (next.color[2] - prev.color[2]),
  ]
}
