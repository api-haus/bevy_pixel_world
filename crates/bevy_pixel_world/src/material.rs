//! Material definitions and registry.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::coords::MaterialId;
use crate::render::{Rgba, rgb};

/// What happens to a pixel under a given effect (burning, detonation, etc.).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PixelEffect {
  /// Pixel is destroyed (set to VOID).
  Destroy,
  /// Pixel transforms into another material.
  Transform(MaterialId),
  /// Pixel resists the effect (no change).
  Resist,
}

/// Per-material effect responses.
#[derive(Clone, Copy, Debug)]
pub struct MaterialEffects {
  /// Burning effect: (effect, per-tick chance). None = no burn
  /// transformation.
  pub on_burn: Option<(PixelEffect, f32)>,
  /// How much blast strength this material absorbs per pixel.
  /// Higher = harder to blast through. 0 = no resistance (void/air).
  pub blast_resistance: f32,
}

/// Physics state determines movement behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PhysicsState {
  /// Does not move.
  Solid,
  /// Falls, piles, slides.
  Powder,
  /// Falls, flows horizontally.
  Liquid,
  /// Rises, disperses.
  Gas,
}

/// Material properties.
pub struct Material {
  pub name: &'static str,
  /// 8-color gradient from surface to deep.
  pub palette: [Rgba; 8],
  /// Physics behavior.
  pub state: PhysicsState,
  /// Density for liquid displacement (higher sinks into lower-density
  /// liquids).
  pub density: u8,
  /// Horizontal spread per tick (liquids).
  pub dispersion: u8,
  /// Air resistance: 1/N chance to skip falling (0 = disabled).
  pub air_resistance: u8,
  /// Air drift: 1/N chance to drift horizontally while falling (0 =
  /// disabled).
  pub air_drift: u8,
  /// Heat level at which this material ignites (0 = non-flammable).
  pub ignition_threshold: u8,
  /// Heat emitted to the heat layer by this material (0 = none).
  pub base_temperature: u8,
  /// Per-material effect responses (burning, detonation, etc.).
  pub effects: MaterialEffects,
}

/// Built-in material IDs.
pub mod ids {
  use super::MaterialId;
  pub const VOID: MaterialId = MaterialId(0);
  pub const SOIL: MaterialId = MaterialId(1);
  pub const STONE: MaterialId = MaterialId(2);
  pub const SAND: MaterialId = MaterialId(3);
  pub const WATER: MaterialId = MaterialId(4);
  pub const WOOD: MaterialId = MaterialId(5);
  pub const ASH: MaterialId = MaterialId(6);
}

use ids::*;

/// Material registry with built-in definitions.
#[derive(bevy::prelude::Resource)]
pub struct Materials {
  entries: Vec<Material>,
}

impl Materials {
  pub fn new() -> Self {
    Self {
      entries: vec![
        // VOID (transparent) - density 0 means everything sinks through
        Material {
          name: "Void",
          palette: [Rgba::new(135, 206, 235, 0); 8], // sky blue, transparent
          state: PhysicsState::Gas,
          density: 0,
          dispersion: 0,
          air_resistance: 0,
          air_drift: 0,
          ignition_threshold: 0,
          base_temperature: 0,
          effects: MaterialEffects {
            on_burn: None,
            blast_resistance: 0.0,
          },
        },
        // SOIL (brown gradient) - powder that falls and piles
        Material {
          name: "Soil",
          palette: [
            rgb(139, 90, 43), // surface - lighter brown
            rgb(130, 82, 38),
            rgb(121, 74, 33),
            rgb(112, 66, 28),
            rgb(103, 58, 23),
            rgb(94, 50, 18),
            rgb(85, 42, 13),
            rgb(76, 34, 8), // deep - darker brown
          ],
          state: PhysicsState::Powder,
          density: 150,
          dispersion: 0,
          air_resistance: 12, // heavier, less floaty
          air_drift: 6,
          ignition_threshold: 0,
          base_temperature: 0,
          effects: MaterialEffects {
            on_burn: None,
            blast_resistance: 0.5,
          },
        },
        // STONE (gray gradient) - solid, does not move
        Material {
          name: "Stone",
          palette: [
            rgb(128, 128, 128), // surface - lighter gray
            rgb(118, 118, 118),
            rgb(108, 108, 108),
            rgb(98, 98, 98),
            rgb(88, 88, 88),
            rgb(78, 78, 78),
            rgb(68, 68, 68),
            rgb(58, 58, 58), // deep - darker gray
          ],
          state: PhysicsState::Solid,
          density: 200,
          dispersion: 0,
          air_resistance: 0,
          air_drift: 0,
          ignition_threshold: 0,
          base_temperature: 0,
          effects: MaterialEffects {
            on_burn: None,
            blast_resistance: 5.0,
          },
        },
        // SAND (tan/yellow gradient) - powder that falls and piles
        Material {
          name: "Sand",
          palette: [
            rgb(237, 201, 175), // surface - light tan
            rgb(225, 191, 146),
            rgb(218, 180, 130),
            rgb(210, 170, 115),
            rgb(200, 160, 100),
            rgb(190, 150, 85),
            rgb(180, 140, 70),
            rgb(170, 130, 60), // deep - darker tan
          ],
          state: PhysicsState::Powder,
          density: 160,
          dispersion: 0,
          air_resistance: 8, // light particles float a bit
          air_drift: 4,      // blown around by wind
          ignition_threshold: 0,
          base_temperature: 0,
          effects: MaterialEffects {
            on_burn: None,
            blast_resistance: 0.3,
          },
        },
        // WATER (blue gradient) - liquid that flows
        Material {
          name: "Water",
          palette: [
            Rgba::new(64, 164, 223, 180), // surface - lighter blue, semi-transparent
            Rgba::new(55, 145, 205, 190),
            Rgba::new(46, 126, 187, 200),
            Rgba::new(37, 107, 169, 210),
            Rgba::new(28, 88, 151, 220),
            Rgba::new(19, 69, 133, 230),
            Rgba::new(10, 50, 115, 240),
            Rgba::new(5, 35, 100, 250), // deep - darker blue
          ],
          state: PhysicsState::Liquid,
          density: 100,
          dispersion: 5,      // flows horizontally
          air_resistance: 16, // subtle splash effect
          air_drift: 12,
          ignition_threshold: 0,
          base_temperature: 0,
          effects: MaterialEffects {
            on_burn: None,
            blast_resistance: 0.1,
          },
        },
        // WOOD (brown gradient) - solid, does not move
        Material {
          name: "Wood",
          palette: [
            rgb(205, 170, 125), // surface - light wood
            rgb(185, 145, 100),
            rgb(165, 120, 80),
            rgb(145, 100, 65),
            rgb(130, 85, 50),
            rgb(110, 70, 40),
            rgb(90, 55, 30),
            rgb(70, 45, 25), // deep - dark wood grain
          ],
          state: PhysicsState::Solid,
          density: 80, // lighter than stone, floats on water
          dispersion: 0,
          air_resistance: 0,
          air_drift: 0,
          ignition_threshold: 40,
          base_temperature: 0,
          effects: MaterialEffects {
            on_burn: Some((PixelEffect::Transform(ASH), 0.005)),
            blast_resistance: 1.0,
          },
        },
        // ASH (gray powder) - product of burning
        Material {
          name: "Ash",
          palette: [
            rgb(180, 175, 170), // surface - light gray
            rgb(165, 160, 155),
            rgb(150, 145, 140),
            rgb(140, 135, 130),
            rgb(130, 125, 120),
            rgb(120, 115, 110),
            rgb(110, 105, 100),
            rgb(100, 95, 90), // deep - darker gray
          ],
          state: PhysicsState::Powder,
          density: 60,
          dispersion: 0,
          air_resistance: 4, // light, floaty
          air_drift: 3,
          ignition_threshold: 0,
          base_temperature: 0,
          effects: MaterialEffects {
            on_burn: None,
            blast_resistance: 0.1,
          },
        },
      ],
    }
  }

  pub fn get(&self, id: MaterialId) -> &Material {
    &self.entries[id.0 as usize]
  }

  /// Returns the number of registered materials.
  #[must_use]
  pub fn len(&self) -> usize {
    self.entries.len()
  }

  /// Returns true if no materials are registered.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }
}

impl Default for Materials {
  fn default() -> Self {
    Self::new()
  }
}

// ─── Serializable config types ───────────────────────────────────────────────

/// Burn behavior in config form, using material names instead of IDs.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BurnEffectConfig {
  Destroy,
  Transform(String),
}

/// Per-material burn configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BurnConfig {
  pub effect: BurnEffectConfig,
  pub chance: f32,
}

/// Per-material effects configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EffectsConfig {
  #[serde(default)]
  pub on_burn: Option<BurnConfig>,
  #[serde(default)]
  pub blast_resistance: f32,
}

/// A single material definition in config form.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaterialConfig {
  pub name: String,
  /// 8 RGBA colors, each as `[r, g, b, a]`.
  pub palette: Vec<[u8; 4]>,
  pub state: PhysicsState,
  #[serde(default)]
  pub density: u8,
  #[serde(default)]
  pub dispersion: u8,
  #[serde(default)]
  pub air_resistance: u8,
  #[serde(default)]
  pub air_drift: u8,
  #[serde(default)]
  pub ignition_threshold: u8,
  #[serde(default)]
  pub base_temperature: u8,
  #[serde(default)]
  pub effects: Option<EffectsConfig>,
}

/// Format-agnostic materials configuration. Deserialize from TOML, JSON, YAML,
/// etc.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaterialsConfig {
  pub materials: Vec<MaterialConfig>,
}

impl MaterialsConfig {
  /// Returns the built-in default materials as a config struct.
  pub fn builtin() -> Self {
    let defaults = Materials::new();
    let mut materials = Vec::with_capacity(defaults.len());
    for entry in &defaults.entries {
      let palette: Vec<[u8; 4]> = entry
        .palette
        .iter()
        .map(|c| [c.red, c.green, c.blue, c.alpha])
        .collect();

      let on_burn = entry.effects.on_burn.map(|(effect, chance)| {
        let effect = match effect {
          PixelEffect::Destroy => BurnEffectConfig::Destroy,
          PixelEffect::Transform(id) => {
            BurnEffectConfig::Transform(defaults.get(id).name.to_string())
          }
          PixelEffect::Resist => BurnEffectConfig::Destroy, /* shouldn't appear in burn
                                                             * config */
        };
        BurnConfig { effect, chance }
      });

      let effects = if on_burn.is_some() || entry.effects.blast_resistance != 0.0 {
        Some(EffectsConfig {
          on_burn,
          blast_resistance: entry.effects.blast_resistance,
        })
      } else {
        None
      };

      materials.push(MaterialConfig {
        name: entry.name.to_string(),
        palette,
        state: entry.state,
        density: entry.density,
        dispersion: entry.dispersion,
        air_resistance: entry.air_resistance,
        air_drift: entry.air_drift,
        ignition_threshold: entry.ignition_threshold,
        base_temperature: entry.base_temperature,
        effects,
      });
    }
    Self { materials }
  }
}

impl From<MaterialsConfig> for Materials {
  fn from(config: MaterialsConfig) -> Self {
    // Build name → index map for resolving cross-references.
    let name_to_index: HashMap<String, u8> = config
      .materials
      .iter()
      .enumerate()
      .map(|(i, m)| (m.name.clone(), i as u8))
      .collect();

    let entries = config
      .materials
      .into_iter()
      .map(|mc| {
        let mut palette = [Rgba::new(0, 0, 0, 0); 8];
        for (i, rgba) in mc.palette.iter().enumerate().take(8) {
          palette[i] = Rgba::new(rgba[0], rgba[1], rgba[2], rgba[3]);
        }

        let effects = match mc.effects {
          Some(ec) => {
            let on_burn = ec.on_burn.map(|bc| {
              let effect = match bc.effect {
                BurnEffectConfig::Destroy => PixelEffect::Destroy,
                BurnEffectConfig::Transform(ref name) => {
                  let idx = name_to_index
                    .get(name)
                    .unwrap_or_else(|| panic!("unknown material in burn transform: {name:?}"));
                  PixelEffect::Transform(MaterialId(*idx))
                }
              };
              (effect, bc.chance)
            });
            MaterialEffects {
              on_burn,
              blast_resistance: ec.blast_resistance,
            }
          }
          None => MaterialEffects {
            on_burn: None,
            blast_resistance: 0.0,
          },
        };

        // Leak name to get &'static str (one-time allocation per material).
        let name: &'static str = Box::leak(mc.name.into_boxed_str());

        Material {
          name,
          palette,
          state: mc.state,
          density: mc.density,
          dispersion: mc.dispersion,
          air_resistance: mc.air_resistance,
          air_drift: mc.air_drift,
          ignition_threshold: mc.ignition_threshold,
          base_temperature: mc.base_temperature,
          effects,
        }
      })
      .collect();

    Self { entries }
  }
}
