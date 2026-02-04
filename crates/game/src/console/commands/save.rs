//! Save command for world persistence management.

use bevy::prelude::*;
use bevy_console::{ConsoleCommand, PrintConsoleLine, reply};
use clap::Parser;

use crate::pixel_world::{
  ClearPersistence, PersistenceComplete, ReloadAllChunks, RequestPersistence, ReseedAllChunks,
};

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
      reply!(log, "Saving...");
    }
  }
}

/// System that listens for save completion and prints to console.
///
/// This ensures users see when the save actually finishes, not just when it
/// starts. Critical for preventing data loss when switching to edit mode.
pub fn notify_save_complete(
  mut events: bevy::ecs::message::MessageReader<PersistenceComplete>,
  mut console: bevy::ecs::message::MessageWriter<PrintConsoleLine>,
  mut last_notified: Local<u64>,
) {
  for event in events.read() {
    // Skip if we've already notified for this request (deduplication)
    if event.request_id <= *last_notified {
      continue;
    }
    *last_notified = event.request_id;

    if event.success {
      console.write(PrintConsoleLine::new("World saved.".to_string()));
    } else if let Some(ref error) = event.error {
      console.write(PrintConsoleLine::new(format!("Save failed: {}", error)));
    } else {
      console.write(PrintConsoleLine::new("Save failed.".to_string()));
    }
  }
}
