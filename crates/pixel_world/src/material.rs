//! Material definitions and registry.

use crate::coords::{ColorIndex, MaterialId};
use crate::render::Rgba;

/// Material properties.
pub struct Material {
  pub name: &'static str,
  /// 8-color gradient from surface to deep.
  pub palette: [Rgba; 8],
}

impl Material {
  /// Sample color from palette using ColorIndex.
  pub fn sample(&self, color: ColorIndex) -> Rgba {
    let idx = (color.0 as usize * 7 / 255).min(7);
    self.palette[idx]
  }
}

/// Built-in material IDs.
pub mod ids {
  use super::MaterialId;
  pub const AIR: MaterialId = MaterialId(0);
  pub const SOIL: MaterialId = MaterialId(1);
  pub const STONE: MaterialId = MaterialId(2);
}

/// Material registry with built-in definitions.
#[derive(bevy::prelude::Resource)]
pub struct Materials {
  entries: Vec<Material>,
}

impl Materials {
  pub fn new() -> Self {
    Self {
      entries: vec![
        // AIR (transparent)
        Material {
          name: "Air",
          palette: [Rgba::new(135, 206, 235, 0); 8], // sky blue, transparent
        },
        // SOIL (brown gradient)
        Material {
          name: "Soil",
          palette: [
            Rgba::rgb(139, 90, 43), // surface - lighter brown
            Rgba::rgb(130, 82, 38),
            Rgba::rgb(121, 74, 33),
            Rgba::rgb(112, 66, 28),
            Rgba::rgb(103, 58, 23),
            Rgba::rgb(94, 50, 18),
            Rgba::rgb(85, 42, 13),
            Rgba::rgb(76, 34, 8), // deep - darker brown
          ],
        },
        // STONE (gray gradient)
        Material {
          name: "Stone",
          palette: [
            Rgba::rgb(128, 128, 128), // surface - lighter gray
            Rgba::rgb(118, 118, 118),
            Rgba::rgb(108, 108, 108),
            Rgba::rgb(98, 98, 98),
            Rgba::rgb(88, 88, 88),
            Rgba::rgb(78, 78, 78),
            Rgba::rgb(68, 68, 68),
            Rgba::rgb(58, 58, 58), // deep - darker gray
          ],
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
}

impl Default for Materials {
  fn default() -> Self {
    Self::new()
  }
}
