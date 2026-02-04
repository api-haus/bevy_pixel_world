//! Compression utilities for chunk persistence.
//!
//! Provides LZ4 compression and delta encoding for efficient storage:
//! - LZ4 for fast decompression (prioritizes load speed)
//! - Delta encoding for chunks with sparse modifications

use crate::pixel_world::ChunkPos;
use crate::pixel_world::coords::CHUNK_SIZE;
use crate::pixel_world::pixel::Pixel;
use crate::pixel_world::primitives::Chunk;
use crate::pixel_world::seeding::ChunkSeeder;

/// Maximum pixels in a chunk (512 * 512 = 262,144).
const MAX_PIXELS: usize = (CHUNK_SIZE * CHUNK_SIZE) as usize;

/// Threshold for delta encoding (as fraction of chunk pixels).
/// Use delta when modifications are below this threshold.
pub const DELTA_THRESHOLD: f32 = 0.75;

/// Compresses raw chunk data using LZ4.
pub fn compress_lz4(data: &[u8]) -> Vec<u8> {
  lz4_flex::compress_prepend_size(data)
}

/// Decompresses LZ4 data.
pub fn decompress_lz4(data: &[u8]) -> Result<Vec<u8>, lz4_flex::block::DecompressError> {
  lz4_flex::decompress_size_prepended(data)
}

/// Delta entry: position (24-bit linear index) + pixel value (32-bit).
///
/// Format: [position_lo, position_mid, position_hi, pixel bytes...]
#[derive(Clone, Copy, Debug)]
pub struct DeltaEntry {
  /// Linear index into chunk buffer (0 to 262,143).
  pub position: u32,
  /// New pixel value.
  pub pixel: Pixel,
}

impl DeltaEntry {
  /// Entry size in bytes (3 for position + 4 for pixel).
  pub const SIZE: usize = 7;

  /// Creates a new delta entry.
  pub fn new(position: u32, pixel: Pixel) -> Self {
    Self { position, pixel }
  }

  /// Writes the entry to a byte buffer.
  pub fn write_to(&self, buf: &mut Vec<u8>) {
    // 24-bit position (little-endian)
    buf.push((self.position & 0xFF) as u8);
    buf.push(((self.position >> 8) & 0xFF) as u8);
    buf.push(((self.position >> 16) & 0xFF) as u8);
    // 4-byte pixel
    buf.push(self.pixel.material.0);
    buf.push(self.pixel.color.0);
    buf.push(self.pixel.damage);
    buf.push(self.pixel.flags_bits());
  }

  /// Reads an entry from a byte slice.
  pub fn read_from(data: &[u8]) -> Option<Self> {
    if data.len() < Self::SIZE {
      return None;
    }
    let position = (data[0] as u32) | ((data[1] as u32) << 8) | ((data[2] as u32) << 16);
    let mut pixel = Pixel {
      material: crate::pixel_world::coords::MaterialId(data[3]),
      color: crate::pixel_world::coords::ColorIndex(data[4]),
      damage: data[5],
      flags: crate::pixel_world::pixel::PixelFlags::empty(),
    };
    pixel.set_flags_bits(data[6]);
    Some(Self { position, pixel })
  }
}

/// Computes delta between current chunk and procedurally generated baseline.
///
/// Returns delta entries for pixels that differ from the seeded baseline.
pub fn compute_delta<S: ChunkSeeder>(chunk: &Chunk, pos: ChunkPos, seeder: &S) -> Vec<DeltaEntry> {
  // Generate baseline chunk
  let mut baseline = Chunk::new(CHUNK_SIZE, CHUNK_SIZE);
  baseline.set_pos(pos);
  seeder.seed(pos, &mut baseline);

  let mut deltas = Vec::new();
  let width = CHUNK_SIZE as usize;

  for y in 0..CHUNK_SIZE {
    for x in 0..CHUNK_SIZE {
      let current = chunk.pixels[(x, y)];
      let base = baseline.pixels[(x, y)];

      if current != base {
        let position = (y as usize * width + x as usize) as u32;
        deltas.push(DeltaEntry::new(position, current));
      }
    }
  }

  deltas
}

/// Encodes delta entries to compressed bytes.
///
/// Format:
/// - Entry count (4 bytes, little-endian)
/// - Delta entries (7 bytes each), LZ4 compressed
pub fn encode_delta(deltas: &[DeltaEntry]) -> Vec<u8> {
  let mut raw = Vec::with_capacity(4 + deltas.len() * DeltaEntry::SIZE);

  // Entry count
  let count = deltas.len() as u32;
  raw.extend_from_slice(&count.to_le_bytes());

  // Entries
  for delta in deltas {
    delta.write_to(&mut raw);
  }

  compress_lz4(&raw)
}

/// Decodes delta entries from compressed bytes.
pub fn decode_delta(data: &[u8]) -> Result<Vec<DeltaEntry>, DeltaError> {
  let raw = decompress_lz4(data).map_err(|_| DeltaError::DecompressionFailed)?;

  if raw.len() < 4 {
    return Err(DeltaError::TooShort);
  }

  let count = u32::from_le_bytes(raw[0..4].try_into().unwrap()) as usize;
  let expected_len = 4 + count * DeltaEntry::SIZE;

  if raw.len() < expected_len {
    return Err(DeltaError::TooShort);
  }

  let mut deltas = Vec::with_capacity(count);
  for i in 0..count {
    let offset = 4 + i * DeltaEntry::SIZE;
    let entry = DeltaEntry::read_from(&raw[offset..]).ok_or(DeltaError::InvalidEntry)?;

    // Validate position is in bounds
    if entry.position >= MAX_PIXELS as u32 {
      return Err(DeltaError::PositionOutOfBounds(entry.position));
    }

    deltas.push(entry);
  }

  Ok(deltas)
}

/// Applies delta entries to a chunk.
pub fn apply_delta(chunk: &mut Chunk, deltas: &[DeltaEntry]) {
  let width = CHUNK_SIZE as usize;

  for delta in deltas {
    let x = (delta.position as usize % width) as u32;
    let y = (delta.position as usize / width) as u32;
    chunk.pixels[(x, y)] = delta.pixel;
  }
}

/// Encodes a full chunk to compressed bytes.
pub fn encode_full(chunk: &Chunk) -> Vec<u8> {
  compress_lz4(chunk.pixels.as_bytes())
}

/// Decodes a full chunk from compressed bytes.
pub fn decode_full(data: &[u8], chunk: &mut Chunk) -> Result<(), FullDecodeError> {
  let raw = decompress_lz4(data).map_err(|_| FullDecodeError::DecompressionFailed)?;

  let expected_size = MAX_PIXELS * std::mem::size_of::<Pixel>();
  if raw.len() != expected_size {
    return Err(FullDecodeError::SizeMismatch {
      expected: expected_size,
      actual: raw.len(),
    });
  }

  // Copy raw bytes into pixel buffer
  // SAFETY: Pixel is repr(C) and the sizes match
  let pixel_bytes = chunk.pixels.as_bytes();
  let pixel_ptr = pixel_bytes.as_ptr() as *mut u8;
  unsafe {
    std::ptr::copy_nonoverlapping(raw.as_ptr(), pixel_ptr, raw.len());
  }

  Ok(())
}

/// Returns whether delta encoding is beneficial for the given modification
/// count.
pub fn should_use_delta(delta_count: usize) -> bool {
  (delta_count as f32) < (MAX_PIXELS as f32 * DELTA_THRESHOLD)
}

/// Delta encoding errors.
#[derive(Debug)]
pub enum DeltaError {
  DecompressionFailed,
  TooShort,
  InvalidEntry,
  PositionOutOfBounds(u32),
}

impl std::fmt::Display for DeltaError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::DecompressionFailed => write!(f, "delta decompression failed"),
      Self::TooShort => write!(f, "delta data too short"),
      Self::InvalidEntry => write!(f, "invalid delta entry"),
      Self::PositionOutOfBounds(p) => write!(f, "delta position out of bounds: {}", p),
    }
  }
}

impl std::error::Error for DeltaError {}

/// Full chunk decoding errors.
#[derive(Debug)]
pub enum FullDecodeError {
  DecompressionFailed,
  SizeMismatch { expected: usize, actual: usize },
}

impl std::fmt::Display for FullDecodeError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::DecompressionFailed => write!(f, "full chunk decompression failed"),
      Self::SizeMismatch { expected, actual } => {
        write!(f, "size mismatch: expected {}, got {}", expected, actual)
      }
    }
  }
}

impl std::error::Error for FullDecodeError {}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::pixel_world::coords::{ColorIndex, MaterialId};

  #[test]
  fn lz4_round_trip() {
    let data = vec![0u8; 1024];
    let compressed = compress_lz4(&data);
    let decompressed = decompress_lz4(&compressed).unwrap();
    assert_eq!(data, decompressed);
  }

  #[test]
  fn delta_entry_round_trip() {
    let entry = DeltaEntry::new(12345, Pixel::new(MaterialId(5), ColorIndex(10)));

    let mut buf = Vec::new();
    entry.write_to(&mut buf);
    assert_eq!(buf.len(), DeltaEntry::SIZE);

    let read = DeltaEntry::read_from(&buf).unwrap();
    assert_eq!(read.position, entry.position);
    assert_eq!(read.pixel, entry.pixel);
  }

  #[test]
  fn delta_encode_decode() {
    let deltas = vec![
      DeltaEntry::new(0, Pixel::new(MaterialId(1), ColorIndex(1))),
      DeltaEntry::new(100, Pixel::new(MaterialId(2), ColorIndex(2))),
      DeltaEntry::new(50000, Pixel::new(MaterialId(3), ColorIndex(3))),
    ];

    let encoded = encode_delta(&deltas);
    let decoded = decode_delta(&encoded).unwrap();

    assert_eq!(decoded.len(), deltas.len());
    for (orig, dec) in deltas.iter().zip(decoded.iter()) {
      assert_eq!(orig.position, dec.position);
      assert_eq!(orig.pixel, dec.pixel);
    }
  }
}
