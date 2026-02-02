//! Shared schedule labels for pixel world systems.
//!
//! All pixel world systems run in [`Update`] within one of the three
//! [`PixelWorldSet`] phases. External consumers can order their own systems
//! relative to these sets.

use bevy::prelude::*;

/// System sets for the pixel world update loop.
///
/// The three phases are chained in order with an [`ApplyDeferred`] barrier
/// between `PreSimulation` and `Simulation`:
///
/// ```text
/// PreSimulation → ApplyDeferred → Simulation → PostSimulation
/// ```
///
/// # Usage
///
/// ```ignore
/// app.add_systems(Update, my_system.in_set(PixelWorldSet::PostSimulation));
/// ```
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum PixelWorldSet {
  /// Chunk streaming, body finalization, persistence message handling.
  PreSimulation,
  /// Cellular automata tick, body blit/readback, splitting.
  Simulation,
  /// Collision generation, body spawning, persistence flush.
  PostSimulation,
}

/// Sub-phases within [`PixelWorldSet::Simulation`].
///
/// These allow body systems to order themselves relative to the CA tick:
///
/// ```text
/// BeforeCATick → CATick → AfterCATick
/// ```
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SimulationPhase {
  /// Body blit and erasure detection, before CA runs.
  BeforeCATick,
  /// The cellular automata simulation tick.
  CATick,
  /// Readback, shape changes, splitting, tile invalidation.
  AfterCATick,
}
