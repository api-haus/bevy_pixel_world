use bevy::prelude::*;
use bevy::time::{Timer, TimerMode};

use crate::PersistenceControl;

pub struct BasicPersistencePlugin;

impl Plugin for BasicPersistencePlugin {
  fn build(&self, app: &mut App) {
    app.add_systems(Update, (handle_save_hotkey, auto_save_system));
  }
}

fn handle_save_hotkey(
  keys: Res<ButtonInput<KeyCode>>,
  persistence: Option<ResMut<PersistenceControl>>,
) {
  let Some(mut persistence) = persistence else {
    return;
  };
  let ctrl_pressed = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
  let s_pressed = keys.just_pressed(KeyCode::KeyS);

  if ctrl_pressed && s_pressed {
    let handle = persistence.save("world");
    info!("Manual save requested (id: {})", handle.id());
  }
}

fn auto_save_system(
  time: Res<Time>,
  mut timer: Local<Option<Timer>>,
  persistence: Option<ResMut<PersistenceControl>>,
) {
  let Some(mut persistence) = persistence else {
    return;
  };
  let timer = timer.get_or_insert_with(|| Timer::from_seconds(5.0, TimerMode::Repeating));
  timer.tick(time.delta());

  if timer.just_finished() {
    persistence.save("world");
  }
}
