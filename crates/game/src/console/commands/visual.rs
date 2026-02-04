//! Visual debug toggle command.

use bevy::prelude::*;
use bevy_console::{ConsoleCommand, reply};
use clap::{Parser, ValueEnum};

use crate::pixel_world::visual_debug::VisualDebugSettings;

#[derive(Clone, Copy, ValueEnum)]
pub enum VisualTarget {
  /// Toggle dirty rect visualization
  DirtyRects,
  /// Toggle collision mesh visualization
  Collision,
}

#[derive(Parser, ConsoleCommand)]
#[command(name = "visual")]
pub struct VisualCommand {
  /// What to toggle
  target: VisualTarget,
}

pub fn visual_command(
  mut log: ConsoleCommand<VisualCommand>,
  mut settings: Option<ResMut<VisualDebugSettings>>,
) {
  if let Some(Ok(VisualCommand { target })) = log.take() {
    let Some(ref mut settings) = settings else {
      reply!(log, "VisualDebugSettings not available");
      return;
    };

    match target {
      VisualTarget::DirtyRects => {
        settings.show_dirty_rects = !settings.show_dirty_rects;
        let state = if settings.show_dirty_rects {
          "on"
        } else {
          "off"
        };
        reply!(log, "Dirty rects: {}", state);
      }
      VisualTarget::Collision => {
        settings.show_collision_meshes = !settings.show_collision_meshes;
        let state = if settings.show_collision_meshes {
          "on"
        } else {
          "off"
        };
        reply!(log, "Collision meshes: {}", state);
      }
    }
  }
}
