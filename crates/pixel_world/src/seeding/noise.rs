//! Noise-based chunk seeding using fastnoise2.

use fastnoise2::generator::prelude::{Generator, GeneratorWrapper};
use fastnoise2::generator::simplex::supersimplex_scaled;
use fastnoise2::SafeNode;

use super::sdf::distance_to_air;
use super::ChunkSeeder;
use crate::coords::{ColorIndex, CHUNK_SIZE};
use crate::material::ids as material_ids;
use crate::pixel::Pixel;
use crate::primitives::Surface;
use crate::{Chunk, ChunkPos};

/// Procedural chunk seeder using coherent noise (grayscale output).
///
/// Generates deterministic terrain based on world position using SuperSimplex
/// noise. Same position always produces identical results.
///
/// This seeder outputs grayscale pixels directly to the render buffer.
/// For material-based terrain, use [`MaterialSeeder`].
#[derive(bevy::prelude::Resource)]
pub struct NoiseSeeder {
  noise: GeneratorWrapper<SafeNode>,
  seed: i32,
}

impl NoiseSeeder {
  /// Creates a new noise seeder.
  ///
  /// - `seed`: Deterministic seed for noise generation.
  /// - `feature_scale`: Controls feature size (larger = larger features).
  pub fn new(seed: i32, feature_scale: f32) -> Self {
    let noise = supersimplex_scaled(feature_scale).build();
    Self { noise, seed }
  }
}

impl ChunkSeeder for NoiseSeeder {
  fn seed(&self, pos: ChunkPos, chunk: &mut Chunk) {
    let base_x = pos.0 as f32 * CHUNK_SIZE as f32;
    let base_y = pos.1 as f32 * CHUNK_SIZE as f32;

    for ly in 0..CHUNK_SIZE {
      for lx in 0..CHUNK_SIZE {
        let wx = base_x + lx as f32;
        let wy = base_y + ly as f32;

        // Sample noise and map [-1, 1] to grayscale [0, 255]
        let value = self.noise.gen_single_2d(wx, wy, self.seed);
        let gray = ((value + 1.0) * 0.5 * 255.0) as u8;

        // For NoiseSeeder, we just use STONE material with varying color
        chunk
          .pixels
          .set(lx, ly, Pixel::new(material_ids::STONE, ColorIndex(gray)));
      }
    }
  }
}

/// Material-based terrain seeder with SDF-feathered boundaries.
///
/// Generates pixelated terrain with:
/// - Hard solid/air boundary from primary noise threshold
/// - Soil layer near surface using SDF distance
/// - Stone layer below soil
/// - Noise-feathered material boundaries for natural edges
#[derive(bevy::prelude::Resource)]
pub struct MaterialSeeder {
  /// Primary noise for terrain shape.
  primary: GeneratorWrapper<SafeNode>,
  /// Secondary noise for edge feathering.
  secondary: GeneratorWrapper<SafeNode>,
  seed: i32,
  /// Solid/air cutoff threshold (noise values below this are solid).
  threshold: f32,
  /// Pixels of soil before stone.
  soil_depth: u8,
  /// Secondary noise influence on boundaries.
  feather_scale: f32,
}

impl MaterialSeeder {
  /// Creates a new material seeder.
  ///
  /// - `seed`: Deterministic seed for noise generation.
  /// - `feature_scale`: Controls primary feature size (larger = larger
  ///   features).
  /// - `threshold`: Noise threshold for solid/air boundary (e.g., 0.0).
  /// - `soil_depth`: Pixels of soil before transitioning to stone (e.g., 8).
  /// - `feather_scale`: Secondary noise influence on boundaries (e.g., 3.0).
  pub fn new(
    seed: i32,
    feature_scale: f32,
    threshold: f32,
    soil_depth: u8,
    feather_scale: f32,
  ) -> Self {
    let primary = supersimplex_scaled(feature_scale).build();
    let secondary = supersimplex_scaled(feature_scale * 0.5).build();
    Self {
      primary,
      secondary,
      seed,
      threshold,
      soil_depth,
      feather_scale,
    }
  }
}

impl ChunkSeeder for MaterialSeeder {
  fn seed(&self, pos: ChunkPos, chunk: &mut Chunk) {
    let base_x = pos.0 as f32 * CHUNK_SIZE as f32;
    let base_y = pos.1 as f32 * CHUNK_SIZE as f32;

    // Pass 1: Generate solid mask
    let mut mask = Surface::<u8>::new(CHUNK_SIZE, CHUNK_SIZE);
    for ly in 0..CHUNK_SIZE {
      for lx in 0..CHUNK_SIZE {
        let wx = base_x + lx as f32;
        let wy = base_y + ly as f32;

        let value = self.primary.gen_single_2d(wx, wy, self.seed);
        // 0 = air, 1 = solid
        mask.set(lx, ly, if value < self.threshold { 1 } else { 0 });
      }
    }

    // Pass 2: Compute SDF (distance to air)
    let sdf = distance_to_air(&mask);

    // Pass 3: Assign materials with feathered colors
    for ly in 0..CHUNK_SIZE {
      for lx in 0..CHUNK_SIZE {
        let dist = sdf[(lx, ly)];
        if dist == 0 {
          chunk.pixels.set(lx, ly, Pixel::AIR);
        } else {
          // Sample secondary noise for feathering
          let wx = base_x + lx as f32;
          let wy = base_y + ly as f32;
          let feather = self.secondary.gen_single_2d(wx * 0.5, wy * 0.5, self.seed);

          // Feathered distance
          let fd = dist as f32 + feather * self.feather_scale;

          // Material selection
          let material = if fd < self.soil_depth as f32 {
            material_ids::SOIL
          } else {
            material_ids::STONE
          };

          // Color from feathered depth (0-255)
          let color = ((fd / 32.0) * 255.0).clamp(0.0, 255.0) as u8;

          chunk
            .pixels
            .set(lx, ly, Pixel::new(material, ColorIndex(color)));
        }
      }
    }
  }
}
