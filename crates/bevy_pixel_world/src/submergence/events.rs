//! Event emission for submersion threshold crossing.
//!
//! Emits [`Submerged`] and [`Surfaced`] messages when bodies cross
//! the submersion threshold.

use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;

use super::{Submerged, SubmersionState, Surfaced};

/// Emits submersion messages when bodies cross the threshold.
///
/// Compares current `is_submerged` with `previous_submerged` to detect
/// threshold crossings. Updates `previous_submerged` after emission.
pub fn emit_submersion_events(
  mut submerged_writer: MessageWriter<Submerged>,
  mut surfaced_writer: MessageWriter<Surfaced>,
  mut query: Query<(Entity, &mut SubmersionState)>,
) {
  for (entity, mut state) in query.iter_mut() {
    if state.is_submerged && !state.previous_submerged {
      submerged_writer.write(Submerged {
        entity,
        submerged_fraction: state.submerged_fraction,
      });
    } else if !state.is_submerged && state.previous_submerged {
      surfaced_writer.write(Surfaced { entity });
    }
    state.previous_submerged = state.is_submerged;
  }
}
