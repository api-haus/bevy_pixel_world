//! Noise-based chunk seeding.
//!
//! Uses fastnoise2 on native, JS bridge to Emscripten FastNoise2 on WASM.
//! Both implementations share the same `NoiseNode` API using encoded node
//! trees.

#[cfg(not(target_family = "wasm"))]
mod native;
#[cfg(target_family = "wasm")]
mod wasm;

// Re-export the appropriate NoiseNode for the target
#[cfg(not(target_family = "wasm"))]
pub use native::NoiseNode;
#[cfg(target_family = "wasm")]
pub use wasm::NoiseNode;

use super::ChunkSeeder;
use super::sdf::distance_to_void;
use crate::pixel_world::coords::{CHUNK_SIZE, ColorIndex};
use crate::pixel_world::material::ids as material_ids;
use crate::pixel_world::pixel::Pixel;
use crate::pixel_world::primitives::Surface;
use crate::pixel_world::{Chunk, ChunkPos};

/// Encoded node presets for FastNoise2.
/// Generate these using FastNoise2's NoiseTool application.
pub mod presets {
  /// Simplex noise for terrain generation
  pub const SIMPLEX: &str = "BwAAgEVDCBY@BE";
}

// ─── NoiseSeeder ────────────────────────────────────────────────────────────

/// Procedural chunk seeder using coherent noise (grayscale output).
#[derive(bevy::prelude::Resource)]
pub struct NoiseSeeder {
  noise: NoiseNode,
  seed: i32,
}

impl NoiseSeeder {
  /// Creates a new noise seeder from an encoded node tree.
  pub fn from_encoded(encoded: &str, seed: i32) -> Option<Self> {
    NoiseNode::from_encoded(encoded).map(|noise| Self { noise, seed })
  }

  /// Creates a new noise seeder using the default terrain preset.
  pub fn new(seed: i32) -> Self {
    Self::from_encoded(presets::SIMPLEX, seed).expect("Failed to create noise from SIMPLEX preset")
  }
}

impl ChunkSeeder for NoiseSeeder {
  fn seed(&self, pos: ChunkPos, chunk: &mut Chunk) {
    let base_x = pos.x as f32 * CHUNK_SIZE as f32;
    let base_y = pos.y as f32 * CHUNK_SIZE as f32;
    let count = (CHUNK_SIZE * CHUNK_SIZE) as usize;
    let mut buffer = vec![0.0f32; count];

    self.noise.gen_uniform_grid_2d(
      &mut buffer,
      base_x,
      base_y,
      CHUNK_SIZE as i32,
      CHUNK_SIZE as i32,
      1.0,
      1.0,
      self.seed,
    );

    for (i, &value) in buffer.iter().enumerate() {
      let lx = (i % CHUNK_SIZE as usize) as u32;
      let ly = (i / CHUNK_SIZE as usize) as u32;
      let gray = ((value + 1.0) * 0.5 * 255.0) as u8;
      chunk
        .pixels
        .set(lx, ly, Pixel::new(material_ids::STONE, ColorIndex(gray)));
    }
  }
}

// ─── MaterialSeeder ─────────────────────────────────────────────────────────

/// Material-based terrain seeder with SDF-feathered boundaries.
#[derive(bevy::prelude::Resource)]
pub struct MaterialSeeder {
  primary: NoiseNode,
  seed: i32,
  threshold: f32,
}

impl MaterialSeeder {
  const DEFAULT_THRESHOLD: f32 = 0.0;

  /// Creates a new material seeder from an encoded node tree.
  pub fn from_encoded(encoded: &str, seed: i32) -> Option<Self> {
    NoiseNode::from_encoded(encoded).map(|primary| Self {
      primary,
      seed,
      threshold: Self::DEFAULT_THRESHOLD,
    })
  }

  /// Creates a new material seeder using the default terrain preset.
  pub fn new(seed: i32) -> Self {
    Self::from_encoded(presets::SIMPLEX, seed).expect("Failed to create noise from SIMPLEX preset")
  }

  pub fn threshold(mut self, threshold: f32) -> Self {
    self.threshold = threshold;
    self
  }
}

impl MaterialSeeder {
  fn generate_solid_mask(&self, base_x: f32, base_y: f32) -> Surface<u8> {
    let mut mask = Surface::<u8>::new(CHUNK_SIZE, CHUNK_SIZE);
    let count = (CHUNK_SIZE * CHUNK_SIZE) as usize;
    let mut buffer = vec![0.0f32; count];

    self.primary.gen_uniform_grid_2d(
      &mut buffer,
      base_x,
      base_y,
      CHUNK_SIZE as i32,
      CHUNK_SIZE as i32,
      1.0,
      1.0,
      self.seed,
    );

    for (i, &value) in buffer.iter().enumerate() {
      let lx = (i % CHUNK_SIZE as usize) as u32;
      let ly = (i / CHUNK_SIZE as usize) as u32;
      mask.set(lx, ly, if value < self.threshold { 1 } else { 0 });
    }
    mask
  }

  fn assign_materials(&self, chunk: &mut Chunk, sdf: &Surface<u8>) {
    for ly in 0..CHUNK_SIZE {
      for lx in 0..CHUNK_SIZE {
        let dist = sdf[(lx, ly)];
        let pixel = if dist == 0 {
          Pixel::VOID
        } else {
          let color = ((dist as f32 / 32.0) * 255.0).clamp(0.0, 255.0) as u8;
          Pixel::new(material_ids::STONE, ColorIndex(color))
        };
        chunk.pixels.set(lx, ly, pixel);
      }
    }
  }
}

impl ChunkSeeder for MaterialSeeder {
  fn seed(&self, pos: ChunkPos, chunk: &mut Chunk) {
    let base_x = pos.x as f32 * CHUNK_SIZE as f32;
    let base_y = pos.y as f32 * CHUNK_SIZE as f32;

    let mask = self.generate_solid_mask(base_x, base_y);
    let sdf = distance_to_void(&mask);
    self.assign_materials(chunk, &sdf);
  }
}
