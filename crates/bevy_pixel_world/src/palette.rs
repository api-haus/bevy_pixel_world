//! Global 256-color palette system with hot-reload support.
//!
//! Provides a global palette that maps ColorIndex (0-255) directly to colors.
//! Includes a 16MB LUT for fast RGB→palette index mapping at sprite load time.

use std::hash::{Hash, Hasher};

use bevy::asset::{Asset, AssetLoader, RenderAssetUsages, io::Reader};
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::tasks::Task;
use lz4_flex::{compress_prepend_size, decompress_size_prepended};
use palette::{IntoColor, Oklab, Srgb};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::material::Materials;
use crate::render::Rgba;

/// Distance function used for RGB→palette mapping.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DistanceFunction {
  /// Euclidean distance in RGB space. Fast but perceptually inaccurate.
  Rgb,
  /// Distance in HSL space. Better for hue preservation.
  Hsl,
  /// Distance in OkLab space. Perceptually uniform, best quality.
  #[default]
  Oklab,
}

/// Dithering mode for RGB→palette mapping.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DitherMode {
  /// Map each pixel to the single nearest palette color.
  #[default]
  Nearest,
  /// Use 2x2 Bayer dithering to pick from candidates.
  Dither,
}

/// Source for palette colors.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PaletteSource {
  /// Inline hex color array.
  Colors { colors: Vec<String> },
  /// Reference to an image file (reads first 256 pixels row-major).
  Image { image: String },
}

/// LUT generation configuration.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LutConfig {
  /// Distance function for color matching.
  #[serde(default)]
  pub distance: DistanceFunction,
  /// Dithering mode.
  #[serde(default)]
  pub mode: DitherMode,
}

/// In-flight LUT computation task.
pub struct LutTask {
  task: Task<Box<[u8; 16_777_216]>>,
  /// Config used to spawn this task (for detecting config changes).
  pub config: LutConfig,
}

/// Configuration for the global palette, loaded from TOML.
#[derive(Asset, TypePath, Clone, Debug, Serialize, Deserialize)]
pub struct PaletteConfig {
  /// Palette color source.
  pub palette: PaletteSource,
  /// LUT generation options.
  #[serde(default)]
  pub lut: LutConfig,
}

/// Asset loader for PaletteConfig TOML files.
#[derive(Default)]
pub struct PaletteConfigLoader;

impl AssetLoader for PaletteConfigLoader {
  type Asset = PaletteConfig;
  type Settings = ();
  type Error = std::io::Error;

  async fn load(
    &self,
    reader: &mut dyn Reader,
    _settings: &Self::Settings,
    _load_context: &mut bevy::asset::LoadContext<'_>,
  ) -> Result<Self::Asset, Self::Error> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await?;
    let config: PaletteConfig = toml::from_str(
      std::str::from_utf8(&bytes)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?,
    )
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(config)
  }

  fn extensions(&self) -> &[&str] {
    &["palette.toml"]
  }
}

/// Global 256-color palette resource.
///
/// Provides direct color lookup via ColorIndex and fast RGB→palette mapping
/// through a precomputed 16MB LUT.
///
/// The LUT is built asynchronously to avoid blocking the main thread during
/// startup. Until the LUT is ready, `map_rgb()` returns `None`.
#[derive(Resource)]
pub struct GlobalPalette {
  /// 256 RGBA colors indexed by ColorIndex.
  pub colors: [Rgba; 256],
  /// 16MB LUT: RGB (24-bit) → palette index (8-bit).
  /// Index = (r << 16) | (g << 8) | b
  /// None until first async build completes.
  lut: Option<Box<[u8; 16_777_216]>>,
  /// In-progress LUT computation task.
  pending_lut: Option<LutTask>,
  /// Handle to the config asset for hot-reload.
  pub config_handle: Option<Handle<PaletteConfig>>,
  /// Triggers GPU re-upload when true.
  pub dirty: bool,
  /// Current LUT configuration (for rebuilding on config change).
  pub lut_config: LutConfig,
  /// Hash of colors + config for the current LUT (for cache validation).
  lut_hash: Option<u64>,
}

impl Default for GlobalPalette {
  fn default() -> Self {
    // Default to a simple grayscale ramp
    let mut colors = [Rgba::new(0, 0, 0, 255); 256];
    for (i, color) in colors.iter_mut().enumerate() {
      *color = Rgba::new(i as u8, i as u8, i as u8, 255);
    }
    // Start with no LUT - call start_lut_build() to begin async computation
    Self {
      colors,
      lut: None,
      pending_lut: None,
      config_handle: None,
      dirty: true,
      lut_config: LutConfig::default(),
      lut_hash: None,
    }
  }
}

impl GlobalPalette {
  /// Creates a palette from a color array.
  ///
  /// The LUT is not built immediately - call `start_lut_build()` to begin
  /// async computation.
  pub fn from_colors(colors: [Rgba; 256], lut_config: LutConfig) -> Self {
    Self {
      colors,
      lut: None,
      pending_lut: None,
      config_handle: None,
      dirty: true,
      lut_config,
      lut_hash: None,
    }
  }

  /// Creates a palette from Materials registry.
  ///
  /// Each material's 8 colors are placed at consecutive palette indices:
  /// material 0 → indices 0-7, material 1 → indices 8-15, etc.
  /// Supports up to 32 materials (256 / 8 = 32).
  ///
  /// The LUT is not built immediately - call `start_lut_build()` to begin
  /// async computation.
  pub fn from_materials(materials: &Materials, lut_config: LutConfig) -> Self {
    let mut colors = [Rgba::new(0, 0, 0, 255); 256];

    let count = materials.len().min(32);
    for material_id in 0..count {
      let material = materials.get(crate::coords::MaterialId(material_id as u8));
      let base = material_id * 8;

      for (color_idx, color) in material.palette.iter().enumerate() {
        let palette_idx = base + color_idx;
        if palette_idx < 256 {
          colors[palette_idx] = *color;
        }
      }
    }

    Self {
      colors,
      lut: None,
      pending_lut: None,
      config_handle: None,
      dirty: true,
      lut_config,
      lut_hash: None,
    }
  }

  /// Maps an RGB color to the nearest palette index using the LUT.
  ///
  /// Returns `None` if the LUT is not yet ready (still building).
  #[inline]
  pub fn map_rgb(&self, r: u8, g: u8, b: u8) -> Option<u8> {
    let lut = self.lut.as_ref()?;
    let idx = ((r as usize) << 16) | ((g as usize) << 8) | (b as usize);
    Some(lut[idx])
  }

  /// Returns true if the LUT is ready for use.
  #[inline]
  pub fn lut_ready(&self) -> bool {
    self.lut.is_some()
  }

  /// Returns true if a LUT build is in progress.
  #[inline]
  pub fn lut_building(&self) -> bool {
    self.pending_lut.is_some()
  }

  /// Gets the color at a palette index.
  #[inline]
  pub fn color(&self, index: u8) -> Rgba {
    self.colors[index as usize]
  }

  /// Starts an async LUT build task.
  ///
  /// If a build is already in progress, it is dropped and a new one starts.
  /// The old LUT (if any) remains usable until the new one completes.
  ///
  /// Note: The async task uses Rayon for parallelization. This is designed
  /// for web builds where we want to avoid blocking the main thread during
  /// startup, but still complete reasonably fast using worker threads.
  pub fn start_lut_build(&mut self) {
    use bevy::tasks::AsyncComputeTaskPool;

    // Copy data for the async task (must be 'static)
    let colors = self.colors;
    let distance_fn = self.lut_config.distance;
    let mode = self.lut_config.mode;
    let config = self.lut_config.clone();

    let task_pool = AsyncComputeTaskPool::get();
    // Use the sequential build_lut to avoid Rayon contention with Bevy's task pool.
    // The parallel version would block an AsyncComputeTaskPool thread while
    // Rayon does its work, potentially starving other async tasks.
    let task = task_pool.spawn(async move { build_lut(&colors, distance_fn, mode) });

    self.pending_lut = Some(LutTask { task, config });
  }

  /// Polls the pending LUT task.
  ///
  /// If `block` is true, cancels the async task and builds synchronously
  /// to avoid contending with other async tasks for compute threads.
  /// Returns `true` if a new LUT just became ready this call.
  pub fn poll_lut(&mut self, block: bool) -> bool {
    let Some(ref mut lut_task) = self.pending_lut else {
      return false;
    };

    if block {
      // In blocking mode, cancel the async task and build synchronously.
      // This avoids starving other async tasks (like chunk seeding) that
      // share the AsyncComputeTaskPool.
      let config = lut_task.config.clone();
      self.pending_lut = None;
      self.lut = Some(build_lut_parallel(
        self.colors,
        config.distance,
        config.mode,
      ));
      self.dirty = true;
      return true;
    }

    if !lut_task.task.is_finished() {
      return false;
    }

    // Task finished, retrieve result
    let new_lut = bevy::tasks::block_on(&mut lut_task.task);
    self.lut = Some(new_lut);
    self.pending_lut = None;
    self.dirty = true;
    true
  }

  /// Rebuilds the LUT synchronously with new configuration.
  ///
  /// Prefer `start_lut_build()` + `poll_lut()` for non-blocking builds.
  pub fn rebuild_lut(&mut self, lut_config: LutConfig) {
    self.lut_config = lut_config.clone();
    self.lut = Some(build_lut(
      &self.colors,
      self.lut_config.distance,
      self.lut_config.mode,
    ));
    self.lut_hash = Some(palette_hash(&self.colors, &lut_config));
    self.pending_lut = None;
    self.dirty = true;
  }

  /// Computes the expected hash for the current palette and config.
  pub fn compute_hash(&self) -> u64 {
    palette_hash(&self.colors, &self.lut_config)
  }

  /// Sets the LUT directly from cached data.
  ///
  /// Used when loading from `assets/lut.bin.lz4`.
  pub fn set_lut_from_cache(&mut self, lut: Box<[u8; 16_777_216]>, hash: u64) {
    self.lut = Some(lut);
    self.lut_hash = Some(hash);
    self.pending_lut = None;
    self.dirty = true;
  }

  /// Returns the LUT data for caching (native builds only).
  pub fn lut_data(&self) -> Option<&[u8; 16_777_216]> {
    self.lut.as_ref().map(|b| b.as_ref())
  }
}

/// Computes a hash of palette colors and LUT configuration.
///
/// Used to detect when the cached LUT needs rebuilding.
pub fn palette_hash(colors: &[Rgba; 256], config: &LutConfig) -> u64 {
  use std::collections::hash_map::DefaultHasher;
  let mut hasher = DefaultHasher::new();
  for c in colors {
    c.red.hash(&mut hasher);
    c.green.hash(&mut hasher);
    c.blue.hash(&mut hasher);
    c.alpha.hash(&mut hasher);
  }
  std::mem::discriminant(&config.distance).hash(&mut hasher);
  std::mem::discriminant(&config.mode).hash(&mut hasher);
  hasher.finish()
}

/// Compresses a 16MB LUT using LZ4.
pub fn compress_lut(lut: &[u8; 16_777_216]) -> Vec<u8> {
  compress_prepend_size(lut)
}

/// Decompresses LUT data.
///
/// Returns `None` if decompression fails or size doesn't match.
pub fn decompress_lut(data: &[u8]) -> Option<Box<[u8; 16_777_216]>> {
  let decompressed = decompress_size_prepended(data).ok()?;
  if decompressed.len() != 16_777_216 {
    return None;
  }
  let ptr = Box::into_raw(decompressed.into_boxed_slice()) as *mut [u8; 16_777_216];
  Some(unsafe { Box::from_raw(ptr) })
}

/// LUT cache file path (relative to assets directory).
pub const LUT_CACHE_PATH: &str = "lut.bin.lz4";

/// Loads a cached LUT from bytes (e.g., from Bevy asset system).
///
/// Format: `[8 bytes: hash][compressed LUT data]`
///
/// Returns `(lut, hash)` if successful.
pub fn load_lut_from_bytes(data: &[u8]) -> Option<(Box<[u8; 16_777_216]>, u64)> {
  if data.len() < 8 {
    return None;
  }
  let hash = u64::from_le_bytes(data[0..8].try_into().ok()?);
  let lut = decompress_lut(&data[8..])?;
  Some((lut, hash))
}

/// Saves a LUT to bytes for caching.
///
/// Format: `[8 bytes: hash][compressed LUT data]`
pub fn save_lut_to_bytes(lut: &[u8; 16_777_216], hash: u64) -> Vec<u8> {
  let compressed = compress_lut(lut);
  let mut result = Vec::with_capacity(8 + compressed.len());
  result.extend_from_slice(&hash.to_le_bytes());
  result.extend_from_slice(&compressed);
  result
}

/// Builds the 16MB RGB→palette LUT.
pub fn build_lut(
  colors: &[Rgba; 256],
  distance_fn: DistanceFunction,
  mode: DitherMode,
) -> Box<[u8; 16_777_216]> {
  // Preallocate the 16MB buffer
  let mut lut = vec![0u8; 16_777_216].into_boxed_slice();

  // Precompute palette colors in the target color space
  let palette_oklab: Vec<Oklab> = colors
    .iter()
    .map(|c| {
      let srgb = Srgb::new(
        c.red as f32 / 255.0,
        c.green as f32 / 255.0,
        c.blue as f32 / 255.0,
      );
      srgb.into_color()
    })
    .collect();

  // For dithering, we need the 4 nearest candidates
  let bayer_2x2 = [[0.0, 0.5], [0.75, 0.25]];

  match mode {
    DitherMode::Nearest => {
      for r in 0..=255u8 {
        for g in 0..=255u8 {
          for b in 0..=255u8 {
            let idx = ((r as usize) << 16) | ((g as usize) << 8) | (b as usize);
            lut[idx] = find_nearest(r, g, b, colors, &palette_oklab, distance_fn);
          }
        }
      }
    }
    DitherMode::Dither => {
      // For dithering, store the threshold-selected candidate
      // The position (r%2, g%2) determines which Bayer threshold to use
      for r in 0..=255u8 {
        for g in 0..=255u8 {
          for b in 0..=255u8 {
            let idx = ((r as usize) << 16) | ((g as usize) << 8) | (b as usize);

            // Find 4 nearest candidates and their distances
            let candidates = find_nearest_candidates(r, g, b, colors, &palette_oklab, distance_fn);

            // Use r%2 and g%2 as Bayer matrix coordinates
            let threshold = bayer_2x2[(r % 2) as usize][(g % 2) as usize];

            // Interpolate between candidates based on threshold
            // Pick candidate based on threshold position in distance range
            let best = if threshold < 0.25 {
              candidates[0].0
            } else if threshold < 0.5 {
              candidates[1].0
            } else if threshold < 0.75 {
              candidates[2].0
            } else {
              candidates[3].0
            };

            lut[idx] = best;
          }
        }
      }
    }
  }

  // Convert Vec to fixed-size array
  let ptr = Box::into_raw(lut) as *mut [u8; 16_777_216];
  unsafe { Box::from_raw(ptr) }
}

/// Builds the 16MB RGB→palette LUT using parallel iteration.
///
/// Uses Rayon to parallelize over the R dimension (256 independent chunks).
/// This is significantly faster than the sequential version on multi-core CPUs.
pub fn build_lut_parallel(
  colors: [Rgba; 256],
  distance_fn: DistanceFunction,
  mode: DitherMode,
) -> Box<[u8; 16_777_216]> {
  // Precompute palette colors in OkLab space (used for OkLab distance)
  let palette_oklab: [Oklab; 256] = std::array::from_fn(|i| {
    let c = &colors[i];
    let srgb = Srgb::new(
      c.red as f32 / 255.0,
      c.green as f32 / 255.0,
      c.blue as f32 / 255.0,
    );
    srgb.into_color()
  });

  let bayer_2x2 = [[0.0f32, 0.5], [0.75, 0.25]];

  // Parallel over R dimension (256 independent 65536-byte chunks)
  let lut_vec: Vec<u8> = (0u8..=255)
    .into_par_iter()
    .flat_map_iter(|r| {
      (0u8..=255).flat_map(move |g| {
        (0u8..=255).map(move |b| match mode {
          DitherMode::Nearest => find_nearest(r, g, b, &colors, &palette_oklab, distance_fn),
          DitherMode::Dither => {
            let candidates = find_nearest_candidates(r, g, b, &colors, &palette_oklab, distance_fn);
            let threshold = bayer_2x2[(r % 2) as usize][(g % 2) as usize];
            if threshold < 0.25 {
              candidates[0].0
            } else if threshold < 0.5 {
              candidates[1].0
            } else if threshold < 0.75 {
              candidates[2].0
            } else {
              candidates[3].0
            }
          }
        })
      })
    })
    .collect();

  // Convert to fixed-size array
  let ptr = Box::into_raw(lut_vec.into_boxed_slice()) as *mut [u8; 16_777_216];
  unsafe { Box::from_raw(ptr) }
}

/// Finds the nearest palette index to an RGB color.
fn find_nearest(
  r: u8,
  g: u8,
  b: u8,
  colors: &[Rgba; 256],
  palette_oklab: &[Oklab],
  distance_fn: DistanceFunction,
) -> u8 {
  let mut best_idx = 0u8;
  let mut best_dist = f32::MAX;

  match distance_fn {
    DistanceFunction::Rgb => {
      let rf = r as f32;
      let gf = g as f32;
      let bf = b as f32;
      for (i, c) in colors.iter().enumerate() {
        let dr = rf - c.red as f32;
        let dg = gf - c.green as f32;
        let db = bf - c.blue as f32;
        let dist = dr * dr + dg * dg + db * db;
        if dist < best_dist {
          best_dist = dist;
          best_idx = i as u8;
        }
      }
    }
    DistanceFunction::Hsl => {
      // Convert source to HSL-ish (simplified: use hue angle distance + lightness)
      let (h, s, l) = rgb_to_hsl(r, g, b);
      for (i, c) in colors.iter().enumerate() {
        let (h2, s2, l2) = rgb_to_hsl(c.red, c.green, c.blue);
        // Hue is circular, compute minimal angular distance
        let dh = {
          let diff = (h - h2).abs();
          if diff > 0.5 { 1.0 - diff } else { diff }
        };
        let ds = s - s2;
        let dl = l - l2;
        // Weight hue more heavily when saturation is high
        let hue_weight = (s + s2) / 2.0;
        let dist = dh * dh * hue_weight * 4.0 + ds * ds + dl * dl * 2.0;
        if dist < best_dist {
          best_dist = dist;
          best_idx = i as u8;
        }
      }
    }
    DistanceFunction::Oklab => {
      let srgb = Srgb::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
      let oklab: Oklab = srgb.into_color();
      for (i, p) in palette_oklab.iter().enumerate() {
        let dl = oklab.l - p.l;
        let da = oklab.a - p.a;
        let db = oklab.b - p.b;
        let dist = dl * dl + da * da + db * db;
        if dist < best_dist {
          best_dist = dist;
          best_idx = i as u8;
        }
      }
    }
  }

  best_idx
}

/// Finds the 4 nearest palette indices for dithering.
fn find_nearest_candidates(
  r: u8,
  g: u8,
  b: u8,
  colors: &[Rgba; 256],
  palette_oklab: &[Oklab],
  distance_fn: DistanceFunction,
) -> [(u8, f32); 4] {
  // Track the 4 best candidates
  let mut candidates: [(u8, f32); 4] = [(0, f32::MAX); 4];

  match distance_fn {
    DistanceFunction::Rgb => {
      let rf = r as f32;
      let gf = g as f32;
      let bf = b as f32;
      for (i, c) in colors.iter().enumerate() {
        let dr = rf - c.red as f32;
        let dg = gf - c.green as f32;
        let db = bf - c.blue as f32;
        let dist = dr * dr + dg * dg + db * db;
        insert_candidate(&mut candidates, i as u8, dist);
      }
    }
    DistanceFunction::Hsl => {
      let (h, s, l) = rgb_to_hsl(r, g, b);
      for (i, c) in colors.iter().enumerate() {
        let (h2, s2, l2) = rgb_to_hsl(c.red, c.green, c.blue);
        let dh = {
          let diff = (h - h2).abs();
          if diff > 0.5 { 1.0 - diff } else { diff }
        };
        let ds = s - s2;
        let dl = l - l2;
        let hue_weight = (s + s2) / 2.0;
        let dist = dh * dh * hue_weight * 4.0 + ds * ds + dl * dl * 2.0;
        insert_candidate(&mut candidates, i as u8, dist);
      }
    }
    DistanceFunction::Oklab => {
      let srgb = Srgb::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
      let oklab: Oklab = srgb.into_color();
      for (i, p) in palette_oklab.iter().enumerate() {
        let dl = oklab.l - p.l;
        let da = oklab.a - p.a;
        let db = oklab.b - p.b;
        let dist = dl * dl + da * da + db * db;
        insert_candidate(&mut candidates, i as u8, dist);
      }
    }
  }

  candidates
}

/// Inserts a candidate into the sorted list if it's closer than the worst.
fn insert_candidate(candidates: &mut [(u8, f32); 4], idx: u8, dist: f32) {
  if dist >= candidates[3].1 {
    return;
  }
  // Find insertion point
  let mut pos = 3;
  while pos > 0 && dist < candidates[pos - 1].1 {
    candidates[pos] = candidates[pos - 1];
    pos -= 1;
  }
  candidates[pos] = (idx, dist);
}

/// Converts RGB to HSL (all components in 0.0-1.0 range).
fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
  let rf = r as f32 / 255.0;
  let gf = g as f32 / 255.0;
  let bf = b as f32 / 255.0;

  let max = rf.max(gf).max(bf);
  let min = rf.min(gf).min(bf);
  let l = (max + min) / 2.0;

  if (max - min).abs() < f32::EPSILON {
    return (0.0, 0.0, l);
  }

  let d = max - min;
  let s = if l > 0.5 {
    d / (2.0 - max - min)
  } else {
    d / (max + min)
  };

  let h = if (max - rf).abs() < f32::EPSILON {
    let mut h = (gf - bf) / d;
    if gf < bf {
      h += 6.0;
    }
    h / 6.0
  } else if (max - gf).abs() < f32::EPSILON {
    ((bf - rf) / d + 2.0) / 6.0
  } else {
    ((rf - gf) / d + 4.0) / 6.0
  };

  (h, s, l)
}

/// Parses a hex color string to RGBA.
pub fn parse_hex_color(hex: &str) -> Option<Rgba> {
  let hex = hex.trim_start_matches('#');
  if hex.len() != 6 && hex.len() != 8 {
    return None;
  }
  let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
  let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
  let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
  let a = if hex.len() == 8 {
    u8::from_str_radix(&hex[6..8], 16).ok()?
  } else {
    255
  };
  Some(Rgba::new(r, g, b, a))
}

/// Loads palette colors from hex strings.
pub fn colors_from_hex(hex_colors: &[String]) -> [Rgba; 256] {
  let mut colors = [Rgba::new(0, 0, 0, 255); 256];
  for (i, hex) in hex_colors.iter().enumerate().take(256) {
    if let Some(c) = parse_hex_color(hex) {
      colors[i] = c;
    }
  }
  colors
}

/// Loads palette colors from an image (first 256 pixels, row-major).
pub fn colors_from_image(image: &Image) -> [Rgba; 256] {
  let mut colors = [Rgba::new(0, 0, 0, 255); 256];

  let Some(ref data) = image.data else {
    return colors;
  };

  let pixel_count = (image.width() * image.height()) as usize;
  let bytes_per_pixel = data.len() / pixel_count.max(1);

  for (i, color) in colors.iter_mut().enumerate().take(pixel_count.min(256)) {
    let offset = i * bytes_per_pixel;
    if offset + 4 <= data.len() {
      *color = Rgba::new(
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
      );
    } else if offset + 3 <= data.len() {
      *color = Rgba::new(data[offset], data[offset + 1], data[offset + 2], 255);
    }
  }

  colors
}

/// Creates a 256x1 palette GPU texture.
pub fn create_palette_texture(images: &mut Assets<Image>) -> Handle<Image> {
  let size = Extent3d {
    width: 256,
    height: 1,
    depth_or_array_layers: 1,
  };

  let mut image = Image::new_fill(
    size,
    TextureDimension::D2,
    &[0, 0, 0, 255],
    TextureFormat::Rgba8UnormSrgb,
    RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
  );

  image.sampler = ImageSampler::nearest();
  images.add(image)
}

/// Uploads GlobalPalette colors to a GPU texture.
pub fn upload_palette(palette: &GlobalPalette, image: &mut Image) {
  let Some(ref mut data) = image.data else {
    return;
  };

  // Copy all 256 colors (256 * 4 = 1024 bytes)
  for (i, color) in palette.colors.iter().enumerate() {
    let offset = i * 4;
    if offset + 4 <= data.len() {
      data[offset] = color.red;
      data[offset + 1] = color.green;
      data[offset + 2] = color.blue;
      data[offset + 3] = color.alpha;
    }
  }
}

/// Converts an image's colors to the nearest palette colors.
///
/// Creates a new image with the same dimensions where each pixel's RGB
/// is remapped to the nearest palette color. Alpha is preserved.
///
/// Returns `None` if the palette LUT is not yet ready.
///
/// # Example
/// ```ignore
/// fn palettize_sprite(
///     mut images: ResMut<Assets<Image>>,
///     palette: Res<GlobalPalette>,
///     sprite_handle: Handle<Image>,
/// ) {
///     if let Some(image) = images.get(&sprite_handle) {
///         if let Some(palettized) = palettize_image(image, &palette) {
///             // Use the palettized image...
///         }
///     }
/// }
/// ```
pub fn palettize_image(image: &Image, palette: &GlobalPalette) -> Option<Image> {
  let mut result = image.clone();
  palettize_image_in_place(&mut result, palette)?;
  Some(result)
}

/// Converts an image's colors to the nearest palette colors in-place.
///
/// Each pixel's RGB is remapped to the nearest palette color. Alpha is
/// preserved.
///
/// Returns `None` if the palette LUT is not yet ready, leaving the image
/// unchanged.
pub fn palettize_image_in_place(image: &mut Image, palette: &GlobalPalette) -> Option<()> {
  let width = image.width() as usize;
  let height = image.height() as usize;
  let pixel_count = width * height;

  if pixel_count == 0 {
    return Some(());
  }

  let Some(ref mut data) = image.data else {
    return Some(());
  };

  let bytes_per_pixel = data.len() / pixel_count;
  if bytes_per_pixel < 3 {
    return Some(()); // Need at least RGB
  }

  for i in 0..pixel_count {
    let offset = i * bytes_per_pixel;
    let r = data[offset];
    let g = data[offset + 1];
    let b = data[offset + 2];

    // Find nearest palette color
    let palette_idx = palette.map_rgb(r, g, b)?;
    let pc = palette.colors[palette_idx as usize];

    // Write palettized color (preserve alpha if present)
    data[offset] = pc.red;
    data[offset + 1] = pc.green;
    data[offset + 2] = pc.blue;
    // Keep original alpha if bytes_per_pixel >= 4
  }

  Some(())
}

/// System that palettizes images when they're loaded.
///
/// Add this system to automatically convert loaded images to palette colors.
/// Mark images that should be palettized with the `PalettizeOnLoad` component.
#[derive(Component)]
pub struct PalettizeOnLoad;

/// Plugin for the global palette system.
pub struct PalettePlugin;

impl Plugin for PalettePlugin {
  fn build(&self, app: &mut App) {
    app.init_asset::<PaletteConfig>();
    app.init_asset_loader::<PaletteConfigLoader>();
    app.init_asset::<LutCacheAsset>();
    app.init_asset_loader::<LutCacheAssetLoader>();
  }
}

/// Raw bytes of a cached LUT file (`assets/lut.bin.lz4`).
#[derive(Asset, TypePath)]
pub struct LutCacheAsset(pub Vec<u8>);

/// Asset loader for LUT cache files.
#[derive(Default)]
pub struct LutCacheAssetLoader;

impl AssetLoader for LutCacheAssetLoader {
  type Asset = LutCacheAsset;
  type Settings = ();
  type Error = std::io::Error;

  async fn load(
    &self,
    reader: &mut dyn Reader,
    _settings: &Self::Settings,
    _load_context: &mut bevy::asset::LoadContext<'_>,
  ) -> Result<Self::Asset, Self::Error> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await?;
    Ok(LutCacheAsset(bytes))
  }

  fn extensions(&self) -> &[&str] {
    &["bin.lz4"]
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn lut_compression_roundtrip() {
    // Create a simple LUT pattern
    let mut lut = vec![0u8; 16_777_216].into_boxed_slice();
    for (i, v) in lut.iter_mut().enumerate() {
      *v = (i % 256) as u8;
    }
    let ptr = Box::into_raw(lut) as *mut [u8; 16_777_216];
    let lut: Box<[u8; 16_777_216]> = unsafe { Box::from_raw(ptr) };

    // Compress
    let compressed = compress_lut(&lut);
    assert!(
      compressed.len() < 16_777_216,
      "Compression should reduce size"
    );

    // Decompress
    let decompressed = decompress_lut(&compressed).expect("Decompression should succeed");

    // Verify roundtrip
    assert_eq!(lut.as_ref(), decompressed.as_ref());
  }

  #[test]
  fn lut_save_load_roundtrip() {
    // Create test data
    let mut lut = vec![0u8; 16_777_216].into_boxed_slice();
    for (i, v) in lut.iter_mut().enumerate() {
      *v = ((i * 7) % 256) as u8;
    }
    let ptr = Box::into_raw(lut) as *mut [u8; 16_777_216];
    let lut: Box<[u8; 16_777_216]> = unsafe { Box::from_raw(ptr) };
    let test_hash = 0x1234567890abcdef_u64;

    // Save to bytes
    let bytes = save_lut_to_bytes(&lut, test_hash);

    // Load from bytes
    let (loaded_lut, loaded_hash) = load_lut_from_bytes(&bytes).expect("Load should succeed");

    // Verify
    assert_eq!(loaded_hash, test_hash);
    assert_eq!(lut.as_ref(), loaded_lut.as_ref());
  }

  #[test]
  fn palette_hash_consistency() {
    let colors = [Rgba::new(1, 2, 3, 255); 256];
    let config = LutConfig::default();

    let hash1 = palette_hash(&colors, &config);
    let hash2 = palette_hash(&colors, &config);

    assert_eq!(hash1, hash2, "Same input should produce same hash");
  }

  #[test]
  fn palette_hash_changes_with_color() {
    let colors1 = [Rgba::new(1, 2, 3, 255); 256];
    let mut colors2 = colors1;
    colors2[0] = Rgba::new(4, 5, 6, 255);
    let config = LutConfig::default();

    let hash1 = palette_hash(&colors1, &config);
    let hash2 = palette_hash(&colors2, &config);

    assert_ne!(
      hash1, hash2,
      "Different colors should produce different hash"
    );
  }
}
