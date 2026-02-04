use bevy::prelude::*;
use clap::Parser;

use crate as bevy_console;
use crate::console::ConsoleState;
use crate::ConsoleCommand;

/// Clears the console
#[derive(Parser, ConsoleCommand)]
#[command(name = "clear")]
pub(crate) struct ClearCommand;

pub(crate) fn clear_command(
  mut clear: ConsoleCommand<ClearCommand>,
  mut state: ResMut<ConsoleState>,
) {
  if let Some(Ok(_)) = clear.take() {
    state.scrollback.clear();
  }
}
