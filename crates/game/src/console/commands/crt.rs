//! CRT effect toggle command.

use bevy::prelude::*;
use bevy_console::{ConsoleCommand, reply};
use bevy_crt::CrtConfig;
use clap::Parser;

#[derive(Parser, ConsoleCommand)]
#[command(name = "crt")]
pub struct CrtCommand {
  /// 0 to disable, 1 to enable
  enabled: u8,
}

pub fn crt_command(mut log: ConsoleCommand<CrtCommand>, mut crt_config: ResMut<CrtConfig>) {
  if let Some(Ok(CrtCommand { enabled })) = log.take() {
    crt_config.enabled = enabled != 0;
    if crt_config.enabled {
      reply!(log, "CRT effect enabled");
    } else {
      reply!(log, "CRT effect disabled");
    }
  }
}
