//! Simulation tick rate configuration.

use bevy::prelude::Resource;

/// Configures tick rates for different simulation systems.
///
/// Each system can run at a different TPS (ticks per second). The physics
/// system runs at the base rate, while burning and heat systems run at lower
/// rates determined by the ratio `physics_tps / system_tps`.
#[derive(Resource, Clone)]
pub struct SimulationConfig {
  /// Physics simulation TPS (pixel swaps, falling sand).
  pub physics_tps: f32,
  /// Burning simulation TPS (fire spread, ash transformation).
  pub burning_tps: f32,
  /// Heat simulation TPS (diffusion, ignition checks).
  pub heat_tps: f32,
}

impl Default for SimulationConfig {
  fn default() -> Self {
    Self {
      physics_tps: 60.0,
      burning_tps: 20.0,
      heat_tps: 3.0,
    }
  }
}
