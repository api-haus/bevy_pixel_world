use bevy::prelude::*;
use bevy_yoleck::prelude::YoleckEditorState;

use crate::core::camera::GameCamera;

pub fn editor_keyboard_shortcuts(
  keys: Res<ButtonInput<KeyCode>>,
  editor_state: Res<State<YoleckEditorState>>,
  mut next_state: ResMut<NextState<YoleckEditorState>>,
) {
  match editor_state.get() {
    YoleckEditorState::EditorActive => {
      if keys.just_pressed(KeyCode::F5) {
        next_state.set(YoleckEditorState::GameActive);
      }
    }
    YoleckEditorState::GameActive => {
      if keys.just_pressed(KeyCode::Escape) {
        next_state.set(YoleckEditorState::EditorActive);
      }
    }
  }
}

pub fn editor_camera_pan(
  keys: Res<ButtonInput<KeyCode>>,
  mut camera_query: Query<&mut Transform, With<GameCamera>>,
  time: Res<Time>,
) {
  let Ok(mut transform) = camera_query.single_mut() else {
    return;
  };

  let mut direction = Vec2::ZERO;
  if keys.pressed(KeyCode::KeyW) {
    direction.y += 1.0;
  }
  if keys.pressed(KeyCode::KeyS) {
    direction.y -= 1.0;
  }
  if keys.pressed(KeyCode::KeyA) {
    direction.x -= 1.0;
  }
  if keys.pressed(KeyCode::KeyD) {
    direction.x += 1.0;
  }

  const PAN_SPEED: f32 = 500.0;
  let delta = direction.normalize_or_zero() * PAN_SPEED * time.delta_secs();
  transform.translation.x += delta.x;
  transform.translation.y += delta.y;
}
