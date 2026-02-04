//! Custom `/` key toggle for the console.
//!
//! Behavior:
//! - `/` always toggles console (open -> close, close -> open)
//! - Escape closes the console (but doesn't open it)
//! - `/` cannot be typed in the console input
//! - Backspace manually handled (workaround for egui bug)
//! - Focus is retained on input after command execution

use bevy::ecs::message::MessageWriter;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy_console::{ConsoleOpen, ConsoleState};
use bevy_egui::{EguiContexts, egui};

/// System that handles custom `/` key toggle and Escape to close.
pub fn handle_console_toggle(
  keys: Res<ButtonInput<KeyCode>>,
  console_open: Res<ConsoleOpen>,
  mut contexts: EguiContexts,
  mut keyboard_events: MessageWriter<KeyboardInput>,
) {
  // Handle Escape: close console only (don't open)
  if keys.just_pressed(KeyCode::Escape) && console_open.open {
    // Consume Escape from egui
    if let Ok(ctx) = contexts.ctx_mut() {
      ctx.input_mut(|i| {
        i.consume_key(egui::Modifiers::NONE, egui::Key::Escape);
      });
    }
    // Send synthetic F12 to close via bevy_console's toggle
    keyboard_events.write(KeyboardInput {
      key_code: KeyCode::F12,
      logical_key: Key::F12,
      state: ButtonState::Pressed,
      text: None,
      repeat: false,
      window: Entity::PLACEHOLDER,
    });
    return;
  }

  // Handle `/`: toggle console
  if !keys.just_pressed(KeyCode::Slash) {
    return;
  }

  // Consume "/" from egui so it doesn't get typed in the input
  if console_open.open {
    if let Ok(ctx) = contexts.ctx_mut() {
      ctx.input_mut(|i| {
        i.consume_key(egui::Modifiers::NONE, egui::Key::Slash);
      });
    }
  }

  // Send synthetic F12 to trigger bevy_console's toggle (includes focus handling)
  keyboard_events.write(KeyboardInput {
    key_code: KeyCode::F12,
    logical_key: Key::F12,
    state: ButtonState::Pressed,
    text: None,
    repeat: false,
    window: Entity::PLACEHOLDER,
  });
}

/// Maintains focus on console input after command execution.
///
/// Tracks the focused widget ID while console is open. When focus is lost
/// (e.g., after pressing Enter), re-requests focus on the stored widget.
pub fn maintain_console_focus(
  console_open: Res<ConsoleOpen>,
  mut contexts: EguiContexts,
  mut last_focused: Local<Option<egui::Id>>,
) {
  if !console_open.open {
    *last_focused = None;
    return;
  }

  let Ok(ctx) = contexts.ctx_mut() else { return };

  let current_focus = ctx.memory(|m| m.focused());

  if let Some(id) = current_focus {
    // Store the focused widget ID
    *last_focused = Some(id);
  } else if let Some(id) = *last_focused {
    // Focus was lost, re-request it
    ctx.memory_mut(|m| m.request_focus(id));
  }
}

/// Workaround for Backspace not working in egui TextEdit.
/// Manually delete the last character from ConsoleState buffer.
pub fn handle_backspace_workaround(
  console_open: Res<ConsoleOpen>,
  keys: Res<ButtonInput<KeyCode>>,
  mut console_state: ResMut<ConsoleState>,
  mut contexts: EguiContexts,
) {
  if !console_open.open {
    return;
  }

  if keys.just_pressed(KeyCode::Backspace) {
    // Pop the last character from the buffer
    console_state.buf.pop();

    // Consume the Backspace from egui to prevent double-handling
    if let Ok(ctx) = contexts.ctx_mut() {
      ctx.input_mut(|i| {
        i.consume_key(egui::Modifiers::NONE, egui::Key::Backspace);
      });
    }
  }
}
