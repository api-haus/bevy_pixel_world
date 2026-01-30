//! Save command for world persistence management.

use bevy::prelude::*;
use bevy_console::{ConsoleCommand, reply};
use bevy_pixel_world::{ClearPersistence, ReloadAllChunks, RequestPersistence, ReseedAllChunks};
use clap::Parser;

#[derive(Parser, ConsoleCommand)]
#[command(name = "save")]
pub struct SaveCommand {
  /// Reload world from disk (discard unsaved changes)
  #[arg(short = 'r', long = "reload")]
  reload: bool,

  /// Clear save file and regenerate world
  #[arg(short = 'c', long = "clear")]
  clear: bool,
}

pub fn save_command(
  mut log: ConsoleCommand<SaveCommand>,
  mut save_request: bevy::ecs::message::MessageWriter<RequestPersistence>,
  mut reload: bevy::ecs::message::MessageWriter<ReloadAllChunks>,
  mut reseed: bevy::ecs::message::MessageWriter<ReseedAllChunks>,
  mut clear: bevy::ecs::message::MessageWriter<ClearPersistence>,
) {
  if let Some(Ok(SaveCommand {
    reload: do_reload,
    clear: do_clear,
  })) = log.take()
  {
    if do_reload {
      reload.write(ReloadAllChunks);
      reply!(log, "Reloading world from disk...");
    } else if do_clear {
      clear.write(ClearPersistence);
      reseed.write(ReseedAllChunks);
      reply!(log, "Save file cleared, world regenerating...");
    } else {
      save_request.write(RequestPersistence);
      reply!(log, "Saving world...");
    }
  }
}
