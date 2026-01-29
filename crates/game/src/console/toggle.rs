//! Custom `/` key toggle for the console.
//!
//! Behavior:
//! - `/` when console closed -> open console
//! - `/` when console open and text field focused -> type `/` character
//!   (handled by egui)
//! - `/` when console open but not focused -> close console

use bevy::prelude::*;
use bevy_console::ConsoleOpen;
use bevy_egui::EguiContexts;

/// System that handles custom `/` key toggle behavior.
pub fn handle_console_toggle(
  keys: Res<ButtonInput<KeyCode>>,
  mut console_open: ResMut<ConsoleOpen>,
  mut contexts: EguiContexts,
) {
  if !keys.just_pressed(KeyCode::Slash) {
    return;
  }

  let Ok(ctx) = contexts.ctx_mut() else { return };

  if console_open.open {
    // Console is open - check if text field is focused
    if ctx.wants_keyboard_input() {
      // Text field focused, let egui handle the `/` character
      return;
    }
    // Not focused on text field, close console
    console_open.open = false;
  } else {
    // Console is closed, open it
    console_open.open = true;
  }
}
