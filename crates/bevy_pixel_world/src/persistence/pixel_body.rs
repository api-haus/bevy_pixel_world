//! Pixel body persistence - save/load pixel bodies to disk.
//!
//! Provides binary serialization for pixel bodies, including:
//! - Transform state (position, rotation)
//! - Physics state (velocities)
//! - Pixel data with LZ4 compression
//! - Shape mask with LZ4 compression
//! - Extension data for game-specific components

use std::io::{self, Read, Write};

use bevy::prelude::*;

use super::compression::{compress_lz4, decompress_lz4};
use super::format::PixelBodyRecordHeader;
use crate::coords::ChunkPos;
use crate::pixel::Pixel;
use crate::pixel_body::{LastBlitTransform, PixelBody, PixelBodyId};

/// A pixel body record ready for serialization.
///
/// Contains all the data needed to reconstruct a pixel body entity.
#[derive(Clone)]
pub struct PixelBodyRecord {
  /// Stable ID for this pixel body.
  pub stable_id: u64,
  /// World position.
  pub position: Vec2,
  /// Rotation in radians.
  pub rotation: f32,
  /// Linear velocity.
  pub linear_velocity: Vec2,
  /// Angular velocity.
  pub angular_velocity: f32,
  /// Width of pixel grid.
  pub width: u32,
  /// Height of pixel grid.
  pub height: u32,
  /// Origin offset.
  pub origin: IVec2,
  /// Raw pixel data (uncompressed).
  pub pixel_data: Vec<Pixel>,
  /// Shape mask (uncompressed).
  pub shape_mask: Vec<bool>,
  /// Game-specific extension data.
  pub extension_data: Vec<u8>,
}

impl PixelBodyRecord {
  /// Creates a record from entity components.
  ///
  /// Physics parameters vary by backend: avian2d uses separate linear/angular
  /// velocity components, rapier2d uses a combined Velocity component.
  pub fn from_components(
    body_id: &PixelBodyId,
    body: &PixelBody,
    transform: &Transform,
    #[cfg(feature = "avian2d")] linear_velocity: Option<&avian2d::prelude::LinearVelocity>,
    #[cfg(feature = "avian2d")] angular_velocity: Option<&avian2d::prelude::AngularVelocity>,
    #[cfg(all(feature = "rapier2d", not(feature = "avian2d")))] velocity: Option<
      &bevy_rapier2d::prelude::Velocity,
    >,
    extension_data: Vec<u8>,
  ) -> Self {
    #[cfg(feature = "avian2d")]
    let (linear_velocity, angular_velocity) = (
      linear_velocity.map(|v| v.0).unwrap_or(Vec2::ZERO),
      angular_velocity.map(|v| v.0).unwrap_or(0.0),
    );

    #[cfg(all(feature = "rapier2d", not(feature = "avian2d")))]
    let (linear_velocity, angular_velocity) = (
      velocity.map(|v| v.linvel).unwrap_or(Vec2::ZERO),
      velocity.map(|v| v.angvel).unwrap_or(0.0),
    );

    #[cfg(not(any(feature = "avian2d", feature = "rapier2d")))]
    let (linear_velocity, angular_velocity) = (Vec2::ZERO, 0.0);

    Self {
      stable_id: body_id.0,
      position: transform.translation.truncate(),
      rotation: transform.rotation.to_euler(EulerRot::ZYX).0,
      linear_velocity,
      angular_velocity,
      width: body.width(),
      height: body.height(),
      origin: body.origin,
      pixel_data: body.surface.as_slice().to_vec(),
      shape_mask: body.shape_mask.clone(),
      extension_data,
    }
  }

  /// Creates a record using the blitted transform instead of current physics
  /// transform.
  ///
  /// This ensures the saved position matches where pixels were actually written
  /// to chunks, preventing ghost pixels when bodies are restored.
  pub fn from_components_blitted(
    body_id: &PixelBodyId,
    body: &PixelBody,
    blitted: &LastBlitTransform,
    #[cfg(feature = "avian2d")] linear_velocity: Option<&avian2d::prelude::LinearVelocity>,
    #[cfg(feature = "avian2d")] angular_velocity: Option<&avian2d::prelude::AngularVelocity>,
    #[cfg(all(feature = "rapier2d", not(feature = "avian2d")))] velocity: Option<
      &bevy_rapier2d::prelude::Velocity,
    >,
    extension_data: Vec<u8>,
  ) -> Option<Self> {
    let transform = blitted.transform.as_ref()?;
    let (_, rotation, translation) = transform.to_scale_rotation_translation();

    #[cfg(feature = "avian2d")]
    let (linear_velocity, angular_velocity) = (
      linear_velocity.map(|v| v.0).unwrap_or(Vec2::ZERO),
      angular_velocity.map(|v| v.0).unwrap_or(0.0),
    );

    #[cfg(all(feature = "rapier2d", not(feature = "avian2d")))]
    let (linear_velocity, angular_velocity) = (
      velocity.map(|v| v.linvel).unwrap_or(Vec2::ZERO),
      velocity.map(|v| v.angvel).unwrap_or(0.0),
    );

    #[cfg(not(any(feature = "avian2d", feature = "rapier2d")))]
    let (linear_velocity, angular_velocity) = (Vec2::ZERO, 0.0);

    Some(Self {
      stable_id: body_id.0,
      position: translation.truncate(),
      rotation: rotation.to_euler(EulerRot::ZYX).0,
      linear_velocity,
      angular_velocity,
      width: body.width(),
      height: body.height(),
      origin: body.origin,
      pixel_data: body.surface.as_slice().to_vec(),
      shape_mask: body.shape_mask.clone(),
      extension_data,
    })
  }

  /// Reconstructs a PixelBody component from this record.
  pub fn to_pixel_body(&self) -> PixelBody {
    let mut body = PixelBody::new(self.width, self.height);
    body.origin = self.origin;

    // Copy pixel data
    let slice = body.surface.as_slice_mut();
    let copy_len = slice.len().min(self.pixel_data.len());
    slice[..copy_len].copy_from_slice(&self.pixel_data[..copy_len]);

    // Copy shape mask
    let mask_len = body.shape_mask.len().min(self.shape_mask.len());
    body.shape_mask[..mask_len].copy_from_slice(&self.shape_mask[..mask_len]);

    body
  }

  /// Returns the chunk position for this body's center.
  pub fn chunk_pos(&self) -> ChunkPos {
    let world_x = self.position.x as i64;
    let world_y = self.position.y as i64;
    let (chunk_pos, _) = crate::coords::WorldPos::new(world_x, world_y).to_chunk_and_local();
    chunk_pos
  }

  /// Writes this record to a writer with compression.
  pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
    // Compress pixel data
    let pixel_bytes = unsafe {
      std::slice::from_raw_parts(
        self.pixel_data.as_ptr() as *const u8,
        self.pixel_data.len() * std::mem::size_of::<Pixel>(),
      )
    };
    let compressed_pixels = compress_lz4(pixel_bytes);

    // Compress shape mask (pack bools into bytes)
    let packed_mask = pack_bools(&self.shape_mask);
    let compressed_mask = compress_lz4(&packed_mask);

    // Build header
    let mut header = PixelBodyRecordHeader {
      stable_id: self.stable_id,
      position_x: self.position.x,
      position_y: self.position.y,
      rotation: self.rotation,
      linear_velocity_x: self.linear_velocity.x,
      linear_velocity_y: self.linear_velocity.y,
      angular_velocity: self.angular_velocity,
      width: self.width,
      height: self.height,
      origin_x: self.origin.x,
      origin_y: self.origin.y,
      pixel_data_size: compressed_pixels.len() as u32,
      shape_mask_size: compressed_mask.len() as u32,
      extension_data_size: self.extension_data.len() as u32,
      checksum: 0,
      _reserved: [0; 3],
    };
    header.checksum = header.compute_checksum();

    // Write header and data
    header.write_to(writer)?;
    writer.write_all(&compressed_pixels)?;
    writer.write_all(&compressed_mask)?;
    writer.write_all(&self.extension_data)?;

    Ok(())
  }

  /// Reads a record from a reader.
  pub fn read_from<R: Read>(reader: &mut R) -> Result<Self, PixelBodyReadError> {
    let header = PixelBodyRecordHeader::read_from(reader)?;

    if !header.validate_checksum() {
      return Err(PixelBodyReadError::ChecksumMismatch);
    }

    // Read compressed pixel data
    let mut compressed_pixels = vec![0u8; header.pixel_data_size as usize];
    reader.read_exact(&mut compressed_pixels)?;

    // Read compressed shape mask
    let mut compressed_mask = vec![0u8; header.shape_mask_size as usize];
    reader.read_exact(&mut compressed_mask)?;

    // Read extension data
    let mut extension_data = vec![0u8; header.extension_data_size as usize];
    reader.read_exact(&mut extension_data)?;

    // Decompress pixel data
    let pixel_bytes =
      decompress_lz4(&compressed_pixels).map_err(|_| PixelBodyReadError::DecompressionFailed)?;

    let pixel_count = (header.width as usize) * (header.height as usize);
    let expected_size = pixel_count * std::mem::size_of::<Pixel>();
    if pixel_bytes.len() != expected_size {
      return Err(PixelBodyReadError::SizeMismatch {
        expected: expected_size,
        actual: pixel_bytes.len(),
      });
    }

    // Convert bytes to pixels
    let mut pixel_data = vec![Pixel::VOID; pixel_count];
    unsafe {
      std::ptr::copy_nonoverlapping(
        pixel_bytes.as_ptr(),
        pixel_data.as_mut_ptr() as *mut u8,
        pixel_bytes.len(),
      );
    }

    // Decompress shape mask
    let packed_mask =
      decompress_lz4(&compressed_mask).map_err(|_| PixelBodyReadError::DecompressionFailed)?;
    let shape_mask = unpack_bools(&packed_mask, pixel_count);

    Ok(Self {
      stable_id: header.stable_id,
      position: Vec2::new(header.position_x, header.position_y),
      rotation: header.rotation,
      linear_velocity: Vec2::new(header.linear_velocity_x, header.linear_velocity_y),
      angular_velocity: header.angular_velocity,
      width: header.width,
      height: header.height,
      origin: IVec2::new(header.origin_x, header.origin_y),
      pixel_data,
      shape_mask,
      extension_data,
    })
  }
}

/// Packs a slice of bools into bytes (8 bools per byte).
fn pack_bools(bools: &[bool]) -> Vec<u8> {
  let byte_count = bools.len().div_ceil(8);
  let mut bytes = vec![0u8; byte_count];

  for (i, &b) in bools.iter().enumerate() {
    if b {
      bytes[i / 8] |= 1 << (i % 8);
    }
  }

  bytes
}

/// Unpacks bytes into bools.
fn unpack_bools(bytes: &[u8], count: usize) -> Vec<bool> {
  let mut bools = vec![false; count];

  for (i, b) in bools.iter_mut().enumerate() {
    if i / 8 < bytes.len() {
      *b = (bytes[i / 8] >> (i % 8)) & 1 != 0;
    }
  }

  bools
}

/// Errors that can occur when reading a pixel body record.
#[derive(Debug)]
pub enum PixelBodyReadError {
  Io(io::Error),
  ChecksumMismatch,
  DecompressionFailed,
  SizeMismatch { expected: usize, actual: usize },
}

impl From<io::Error> for PixelBodyReadError {
  fn from(err: io::Error) -> Self {
    Self::Io(err)
  }
}

impl std::fmt::Display for PixelBodyReadError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Io(e) => write!(f, "I/O error: {}", e),
      Self::ChecksumMismatch => write!(f, "checksum mismatch"),
      Self::DecompressionFailed => write!(f, "decompression failed"),
      Self::SizeMismatch { expected, actual } => {
        write!(f, "size mismatch: expected {}, got {}", expected, actual)
      }
    }
  }
}

impl std::error::Error for PixelBodyReadError {}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::coords::{ColorIndex, MaterialId};

  #[test]
  fn pack_unpack_bools() {
    let bools = vec![true, false, true, true, false, false, true, false, true];
    let packed = pack_bools(&bools);
    let unpacked = unpack_bools(&packed, bools.len());
    assert_eq!(bools, unpacked);
  }

  #[test]
  fn pixel_body_record_round_trip() {
    let width = 4;
    let height = 4;
    let pixel_count = (width * height) as usize;

    let record = PixelBodyRecord {
      stable_id: 12345,
      position: Vec2::new(100.5, 200.5),
      rotation: 0.5,
      linear_velocity: Vec2::new(10.0, -5.0),
      angular_velocity: 0.1,
      width,
      height,
      origin: IVec2::new(-2, -2),
      pixel_data: vec![Pixel::new(MaterialId(1), ColorIndex(2)); pixel_count],
      shape_mask: vec![true; pixel_count],
      extension_data: vec![1, 2, 3, 4],
    };

    let mut buf = Vec::new();
    record.write_to(&mut buf).unwrap();

    let mut cursor = std::io::Cursor::new(&buf);
    let read_record = PixelBodyRecord::read_from(&mut cursor).unwrap();

    assert_eq!(read_record.stable_id, record.stable_id);
    assert_eq!(read_record.position, record.position);
    assert_eq!(read_record.rotation, record.rotation);
    assert_eq!(read_record.linear_velocity, record.linear_velocity);
    assert_eq!(read_record.angular_velocity, record.angular_velocity);
    assert_eq!(read_record.width, record.width);
    assert_eq!(read_record.height, record.height);
    assert_eq!(read_record.origin, record.origin);
    assert_eq!(read_record.pixel_data.len(), record.pixel_data.len());
    assert_eq!(read_record.shape_mask, record.shape_mask);
    assert_eq!(read_record.extension_data, record.extension_data);
  }
}
