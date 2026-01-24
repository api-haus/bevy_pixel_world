//! Noise-based chunk seeding using fastnoise2.

use fastnoise2::SafeNode;
use fastnoise2::generator::prelude::{Generator, GeneratorWrapper};
use fastnoise2::generator::simplex::supersimplex_scaled;

use super::ChunkSeeder;
use super::sdf::distance_to_void;
use crate::coords::{CHUNK_SIZE, ColorIndex};
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
    let base_x = pos.x as f32 * CHUNK_SIZE as f32;
    let base_y = pos.y as f32 * CHUNK_SIZE as f32;

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
/// - Hard solid/void boundary from primary noise threshold
/// - Stone base terrain
/// - Surface sand patches and water pools via islet noise
/// - Noise-feathered material boundaries for natural edges
#[derive(bevy::prelude::Resource)]
pub struct MaterialSeeder {
  /// Primary noise for terrain shape.
  primary: GeneratorWrapper<SafeNode>,
  /// Secondary noise for edge feathering.
  secondary: GeneratorWrapper<SafeNode>,
  /// Noise for sand islet placement.
  sand_noise: GeneratorWrapper<SafeNode>,
  /// Noise for water islet placement.
  water_noise: GeneratorWrapper<SafeNode>,
  seed: i32,
  /// Solid/void cutoff threshold (noise values below this are solid).
  threshold: f32,
  /// Secondary noise influence on boundaries.
  feather_scale: f32,
  /// Feature scale for islet noise.
  islet_scale: f32,
  /// Threshold for islet activation (higher = fewer islets).
  islet_threshold: f32,
  /// Max distance from void for islets to appear.
  islet_depth: u8,
}

impl MaterialSeeder {
  /// Default feature scale (controls primary terrain feature size).
  const DEFAULT_FEATURE_SCALE: f32 = 200.0;
  /// Default solid/void threshold.
  const DEFAULT_THRESHOLD: f32 = 0.0;
  /// Default secondary noise influence.
  const DEFAULT_FEATHER_SCALE: f32 = 3.0;
  /// Default islet feature scale.
  const DEFAULT_ISLET_SCALE: f32 = 30.0;
  /// Default islet threshold (higher = fewer islets).
  const DEFAULT_ISLET_THRESHOLD: f32 = 0.6;
  /// Default max depth from void for islets.
  const DEFAULT_ISLET_DEPTH: u8 = 3;

  /// Creates a new material seeder with the given seed and default parameters.
  ///
  /// Use builder methods to customize:
  /// - `feature_scale(f32)`: Controls terrain feature size (default: 200.0)
  /// - `threshold(f32)`: Noise cutoff for solid/air (default: 0.0)
  /// - `feather_scale(f32)`: Edge noise influence (default: 3.0)
  /// - `islet_scale(f32)`: Feature scale for islets (default: 30.0)
  /// - `islet_threshold(f32)`: Threshold for islet activation (default: 0.6)
  /// - `islet_depth(u8)`: Max depth from air for islets (default: 3)
  pub fn new(seed: i32) -> Self {
    let feature_scale = Self::DEFAULT_FEATURE_SCALE;
    let islet_scale = Self::DEFAULT_ISLET_SCALE;
    let primary = supersimplex_scaled(feature_scale).build();
    let secondary = supersimplex_scaled(feature_scale * 0.5).build();
    // Use different seed offsets for sand/water to get distinct patterns
    let sand_noise = supersimplex_scaled(islet_scale).build();
    let water_noise = supersimplex_scaled(islet_scale).build();
    Self {
      primary,
      secondary,
      sand_noise,
      water_noise,
      seed,
      threshold: Self::DEFAULT_THRESHOLD,
      feather_scale: Self::DEFAULT_FEATHER_SCALE,
      islet_scale,
      islet_threshold: Self::DEFAULT_ISLET_THRESHOLD,
      islet_depth: Self::DEFAULT_ISLET_DEPTH,
    }
  }

  /// Sets the primary feature scale (larger = larger terrain features).
  pub fn feature_scale(mut self, scale: f32) -> Self {
    self.primary = supersimplex_scaled(scale).build();
    self.secondary = supersimplex_scaled(scale * 0.5).build();
    self
  }

  /// Sets the solid/void threshold (noise values below this are solid).
  pub fn threshold(mut self, threshold: f32) -> Self {
    self.threshold = threshold;
    self
  }

  /// Sets the secondary noise influence on material boundaries.
  pub fn feather_scale(mut self, scale: f32) -> Self {
    self.feather_scale = scale;
    self
  }

  /// Sets the islet feature scale (controls sand/water patch size).
  pub fn islet_scale(mut self, scale: f32) -> Self {
    self.islet_scale = scale;
    self.sand_noise = supersimplex_scaled(scale).build();
    self.water_noise = supersimplex_scaled(scale).build();
    self
  }

  /// Sets the islet threshold (higher = fewer islets).
  pub fn islet_threshold(mut self, threshold: f32) -> Self {
    self.islet_threshold = threshold;
    self
  }

  /// Sets the max depth from void for islets to appear.
  pub fn islet_depth(mut self, depth: u8) -> Self {
    self.islet_depth = depth;
    self
  }
}

impl ChunkSeeder for MaterialSeeder {
  fn seed(&self, pos: ChunkPos, chunk: &mut Chunk) {
    let base_x = pos.x as f32 * CHUNK_SIZE as f32;
    let base_y = pos.y as f32 * CHUNK_SIZE as f32;

    // Pass 1: Generate solid mask
    let mut mask = Surface::<u8>::new(CHUNK_SIZE, CHUNK_SIZE);
    for ly in 0..CHUNK_SIZE {
      for lx in 0..CHUNK_SIZE {
        let wx = base_x + lx as f32;
        let wy = base_y + ly as f32;

        let value = self.primary.gen_single_2d(wx, wy, self.seed);
        // 0 = void, 1 = solid
        mask.set(lx, ly, if value < self.threshold { 1 } else { 0 });
      }
    }

    // Pass 2: Compute SDF (distance to void)
    let sdf = distance_to_void(&mask);

    // Pass 3: Assign materials with feathered colors
    for ly in 0..CHUNK_SIZE {
      for lx in 0..CHUNK_SIZE {
        let dist = sdf[(lx, ly)];
        if dist == 0 {
          chunk.pixels.set(lx, ly, Pixel::VOID);
        } else {
          // Sample secondary noise for feathering
          let wx = base_x + lx as f32;
          let wy = base_y + ly as f32;
          let feather = self.secondary.gen_single_2d(wx * 0.5, wy * 0.5, self.seed);

          // Feathered distance
          let fd = dist as f32 + feather * self.feather_scale;

          // Material selection: check for surface islets first
          let material = if dist <= self.islet_depth {
            // Surface zone - check for sand/water islets
            // Use different seed offsets for distinct patterns
            let sand_val = self.sand_noise.gen_single_2d(wx, wy, self.seed);
            let water_val = self.water_noise.gen_single_2d(wx, wy, self.seed + 1000);

            if sand_val > self.islet_threshold {
              material_ids::SAND
            } else if water_val > self.islet_threshold {
              material_ids::WATER
            } else {
              material_ids::STONE
            }
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
