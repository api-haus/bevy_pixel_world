//! Noise-based chunk seeding using fastnoise2.

use fastnoise2::generator::prelude::{Generator, GeneratorWrapper};
use fastnoise2::generator::simplex::supersimplex_scaled;
use fastnoise2::SafeNode;

use super::ChunkSeeder;
use crate::coords::CHUNK_SIZE;
use crate::{Chunk, ChunkPos, Rgba};

/// Procedural chunk seeder using coherent noise.
///
/// Generates deterministic terrain based on world position using SuperSimplex noise.
/// Same position always produces identical results.
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

                chunk.pixels.set(lx, ly, Rgba::rgb(gray, gray, gray));
            }
        }
    }
}
