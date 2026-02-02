//! Binary format types for chunk persistence.
//!
//! Defines the on-disk format for save files:
//! - [`Header`]: 64-byte file header with magic, version, and metadata
//! - [`PageTableEntry`]: 24-byte index entry mapping chunk position to data
//!   offset
//! - [`StorageType`]: Compression strategy (Empty, Delta, Full)

use std::io::{self, Read, Write};

use crate::pixel_world::coords::{CHUNK_SIZE, ChunkPos, TILE_SIZE};
use crate::pixel_world::pixel::Pixel;

/// Magic bytes identifying a pixel world save file ("PXSW").
pub const MAGIC: u32 = 0x5053_5857;

/// Current format version.
pub const VERSION: u16 = 1;

/// File header (64 bytes, fixed size).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Header {
  /// Magic number (0x50585357 = "PXSW").
  pub magic: u32,
  /// Format version for migration.
  pub version: u16,
  /// Feature flags (compression type, etc.).
  pub flags: u16,
  /// World seed for procedural regeneration.
  pub world_seed: u64,
  /// Unix timestamp of world creation.
  pub creation_time: u64,
  /// Unix timestamp of last save.
  pub modified_time: u64,
  /// Number of saved chunks.
  pub chunk_count: u32,
  /// Bytes allocated for page table.
  pub page_table_size: u32,
  /// File offset where data region starts.
  pub data_region_ptr: u64,
  /// Pixels per chunk edge.
  pub chunk_size: u16,
  /// Pixels per tile edge.
  pub tile_size: u16,
  /// Bytes per pixel.
  pub pixel_size: u8,
  /// File offset where entity section starts (0 = no entities).
  pub entity_section_ptr: u64,
  /// Reserved for future use.
  pub _reserved: [u8; 3],
}

impl Header {
  /// Header size in bytes.
  pub const SIZE: usize = 64;

  /// Creates a new header with default values.
  pub fn new(world_seed: u64) -> Self {
    #[cfg(not(target_family = "wasm"))]
    let now = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .map(|d| d.as_secs())
      .unwrap_or(0);
    #[cfg(target_family = "wasm")]
    let now = (js_sys::Date::now() / 1000.0) as u64;

    Self {
      magic: MAGIC,
      version: VERSION,
      flags: 0,
      world_seed,
      creation_time: now,
      modified_time: now,
      chunk_count: 0,
      page_table_size: 0,
      data_region_ptr: Self::SIZE as u64, // Initially points to end of header
      chunk_size: CHUNK_SIZE as u16,
      tile_size: TILE_SIZE as u16,
      pixel_size: std::mem::size_of::<Pixel>() as u8,
      entity_section_ptr: 0, // No entities initially
      _reserved: [0; 3],
    }
  }

  /// Validates the header against current game constants.
  pub fn validate(&self) -> Result<(), HeaderError> {
    if self.magic != MAGIC {
      return Err(HeaderError::InvalidMagic(self.magic));
    }
    if self.version > VERSION {
      return Err(HeaderError::UnsupportedVersion(self.version));
    }
    if self.chunk_size != CHUNK_SIZE as u16 {
      return Err(HeaderError::ChunkSizeMismatch {
        file: self.chunk_size,
        game: CHUNK_SIZE as u16,
      });
    }
    if self.tile_size != TILE_SIZE as u16 {
      return Err(HeaderError::TileSizeMismatch {
        file: self.tile_size,
        game: TILE_SIZE as u16,
      });
    }
    if self.pixel_size != std::mem::size_of::<Pixel>() as u8 {
      return Err(HeaderError::PixelSizeMismatch {
        file: self.pixel_size,
        game: std::mem::size_of::<Pixel>() as u8,
      });
    }
    Ok(())
  }

  /// Writes the header to a writer.
  pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
    writer.write_all(&self.magic.to_le_bytes())?;
    writer.write_all(&self.version.to_le_bytes())?;
    writer.write_all(&self.flags.to_le_bytes())?;
    writer.write_all(&self.world_seed.to_le_bytes())?;
    writer.write_all(&self.creation_time.to_le_bytes())?;
    writer.write_all(&self.modified_time.to_le_bytes())?;
    writer.write_all(&self.chunk_count.to_le_bytes())?;
    writer.write_all(&self.page_table_size.to_le_bytes())?;
    writer.write_all(&self.data_region_ptr.to_le_bytes())?;
    writer.write_all(&self.chunk_size.to_le_bytes())?;
    writer.write_all(&self.tile_size.to_le_bytes())?;
    writer.write_all(&[self.pixel_size])?;
    writer.write_all(&self.entity_section_ptr.to_le_bytes())?;
    writer.write_all(&self._reserved)?;
    Ok(())
  }

  /// Reads a header from a reader.
  pub fn read_from<R: Read>(reader: &mut R) -> io::Result<Self> {
    let mut buf = [0u8; Self::SIZE];
    reader.read_exact(&mut buf)?;

    Ok(Self {
      magic: u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]),
      version: u16::from_le_bytes([buf[4], buf[5]]),
      flags: u16::from_le_bytes([buf[6], buf[7]]),
      world_seed: u64::from_le_bytes(buf[8..16].try_into().unwrap()),
      creation_time: u64::from_le_bytes(buf[16..24].try_into().unwrap()),
      modified_time: u64::from_le_bytes(buf[24..32].try_into().unwrap()),
      chunk_count: u32::from_le_bytes(buf[32..36].try_into().unwrap()),
      page_table_size: u32::from_le_bytes(buf[36..40].try_into().unwrap()),
      data_region_ptr: u64::from_le_bytes(buf[40..48].try_into().unwrap()),
      chunk_size: u16::from_le_bytes([buf[48], buf[49]]),
      tile_size: u16::from_le_bytes([buf[50], buf[51]]),
      pixel_size: buf[52],
      entity_section_ptr: u64::from_le_bytes(buf[53..61].try_into().unwrap()),
      _reserved: buf[61..64].try_into().unwrap(),
    })
  }
}

/// Header validation errors.
#[derive(Debug)]
pub enum HeaderError {
  InvalidMagic(u32),
  UnsupportedVersion(u16),
  ChunkSizeMismatch { file: u16, game: u16 },
  TileSizeMismatch { file: u16, game: u16 },
  PixelSizeMismatch { file: u8, game: u8 },
}

impl std::fmt::Display for HeaderError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::InvalidMagic(m) => write!(f, "invalid magic number: 0x{:08X}", m),
      Self::UnsupportedVersion(v) => write!(f, "unsupported version: {}", v),
      Self::ChunkSizeMismatch { file, game } => {
        write!(f, "chunk size mismatch: file={}, game={}", file, game)
      }
      Self::TileSizeMismatch { file, game } => {
        write!(f, "tile size mismatch: file={}, game={}", file, game)
      }
      Self::PixelSizeMismatch { file, game } => {
        write!(f, "pixel size mismatch: file={}, game={}", file, game)
      }
    }
  }
}

impl std::error::Error for HeaderError {}

/// Storage type for chunk data.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StorageType {
  /// Chunk entirely cleared (no data needed).
  #[default]
  Empty = 0,
  /// Stores only changes from procedural generation.
  Delta = 1,
  /// Stores complete chunk buffer.
  Full = 2,
}

impl StorageType {
  /// Converts a byte to a storage type.
  pub fn from_u8(value: u8) -> Option<Self> {
    match value {
      0 => Some(Self::Empty),
      1 => Some(Self::Delta),
      2 => Some(Self::Full),
      _ => None,
    }
  }
}

/// Page table entry (24 bytes).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PageTableEntry {
  /// World chunk X coordinate (signed i32).
  pub chunk_x: i32,
  /// World chunk Y coordinate (signed i32).
  pub chunk_y: i32,
  /// Offset into data region (u64).
  pub data_offset: u64,
  /// Compressed data size in bytes (u32).
  pub data_size: u32,
  /// Storage type (Empty, Delta, Full).
  pub storage_type: StorageType,
  /// CRC8 checksum for corruption detection.
  pub checksum: u8,
  /// Alignment padding.
  pub _reserved: [u8; 2],
}

/// Updates a CRC8 value with a new byte using polynomial 0x07 (CRC-8-CCITT).
fn crc8_update(crc: &mut u8, byte: u8) {
  *crc ^= byte;
  for _ in 0..8 {
    *crc = if *crc & 0x80 != 0 {
      (*crc << 1) ^ 0x07
    } else {
      *crc << 1
    };
  }
}

/// Computes CRC8 checksum over multiple byte slices.
fn checksum_fields(fields: &[&[u8]]) -> u8 {
  let mut crc: u8 = 0;
  for field in fields {
    for &byte in *field {
      crc8_update(&mut crc, byte);
    }
  }
  crc
}

impl PageTableEntry {
  /// Entry size in bytes.
  pub const SIZE: usize = 24;

  /// Creates a new entry for a chunk position.
  pub fn new(pos: ChunkPos, data_offset: u64, data_size: u32, storage_type: StorageType) -> Self {
    let mut entry = Self {
      chunk_x: pos.x,
      chunk_y: pos.y,
      data_offset,
      data_size,
      storage_type,
      checksum: 0,
      _reserved: [0; 2],
    };
    entry.checksum = entry.compute_checksum();
    entry
  }

  /// Returns the chunk position for this entry.
  pub fn pos(&self) -> ChunkPos {
    ChunkPos::new(self.chunk_x, self.chunk_y)
  }

  /// Computes CRC8 checksum of the entry (excluding checksum field).
  pub fn compute_checksum(&self) -> u8 {
    checksum_fields(&[
      &self.chunk_x.to_le_bytes(),
      &self.chunk_y.to_le_bytes(),
      &self.data_offset.to_le_bytes(),
      &self.data_size.to_le_bytes(),
      &[self.storage_type as u8],
    ])
  }

  /// Validates the entry checksum.
  pub fn validate_checksum(&self) -> bool {
    self.checksum == self.compute_checksum()
  }

  /// Writes the entry to a writer.
  pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
    writer.write_all(&self.chunk_x.to_le_bytes())?;
    writer.write_all(&self.chunk_y.to_le_bytes())?;
    writer.write_all(&self.data_offset.to_le_bytes())?;
    writer.write_all(&self.data_size.to_le_bytes())?;
    writer.write_all(&[self.storage_type as u8])?;
    writer.write_all(&[self.checksum])?;
    writer.write_all(&self._reserved)?;
    Ok(())
  }

  /// Reads an entry from a reader.
  pub fn read_from<R: Read>(reader: &mut R) -> io::Result<Self> {
    let mut buf = [0u8; Self::SIZE];
    reader.read_exact(&mut buf)?;

    let storage_type = StorageType::from_u8(buf[20]).unwrap_or(StorageType::Empty);

    Ok(Self {
      chunk_x: i32::from_le_bytes(buf[0..4].try_into().unwrap()),
      chunk_y: i32::from_le_bytes(buf[4..8].try_into().unwrap()),
      data_offset: u64::from_le_bytes(buf[8..16].try_into().unwrap()),
      data_size: u32::from_le_bytes(buf[16..20].try_into().unwrap()),
      storage_type,
      checksum: buf[21],
      _reserved: [buf[22], buf[23]],
    })
  }
}

/// Entity section header (8 bytes).
///
/// Precedes the array of PixelBodyRecord entries.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct EntitySectionHeader {
  /// Number of pixel body records in this section.
  pub entity_count: u32,
  /// Reserved for future use.
  pub _reserved: u32,
}

impl EntitySectionHeader {
  /// Header size in bytes.
  pub const SIZE: usize = 8;

  /// Writes the header to a writer.
  pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
    writer.write_all(&self.entity_count.to_le_bytes())?;
    writer.write_all(&self._reserved.to_le_bytes())?;
    Ok(())
  }

  /// Reads a header from a reader.
  pub fn read_from<R: Read>(reader: &mut R) -> io::Result<Self> {
    let mut buf = [0u8; Self::SIZE];
    reader.read_exact(&mut buf)?;
    Ok(Self {
      entity_count: u32::from_le_bytes(buf[0..4].try_into().unwrap()),
      _reserved: u32::from_le_bytes(buf[4..8].try_into().unwrap()),
    })
  }
}

/// Fixed-size header for a pixel body record (64 bytes).
///
/// The variable-size data (pixel data, shape mask, extension data) follows
/// immediately after this header.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PixelBodyRecordHeader {
  /// Stable ID for this pixel body.
  pub stable_id: u64,
  /// World X position.
  pub position_x: f32,
  /// World Y position.
  pub position_y: f32,
  /// Rotation in radians.
  pub rotation: f32,
  /// Linear velocity X.
  pub linear_velocity_x: f32,
  /// Linear velocity Y.
  pub linear_velocity_y: f32,
  /// Angular velocity.
  pub angular_velocity: f32,
  /// Width of pixel grid.
  pub width: u32,
  /// Height of pixel grid.
  pub height: u32,
  /// Origin X offset.
  pub origin_x: i32,
  /// Origin Y offset.
  pub origin_y: i32,
  /// Size of compressed pixel data.
  pub pixel_data_size: u32,
  /// Size of compressed shape mask data.
  pub shape_mask_size: u32,
  /// Size of extension data.
  pub extension_data_size: u32,
  /// Checksum for corruption detection.
  pub checksum: u8,
  /// Reserved for future use.
  pub _reserved: [u8; 3],
}

impl PixelBodyRecordHeader {
  /// Header size in bytes.
  pub const SIZE: usize = 64;

  /// Computes CRC8 checksum of the header (excluding checksum field).
  pub fn compute_checksum(&self) -> u8 {
    checksum_fields(&[
      &self.stable_id.to_le_bytes(),
      &self.position_x.to_le_bytes(),
      &self.position_y.to_le_bytes(),
      &self.rotation.to_le_bytes(),
      &self.width.to_le_bytes(),
      &self.height.to_le_bytes(),
    ])
  }

  /// Validates the header checksum.
  pub fn validate_checksum(&self) -> bool {
    self.checksum == self.compute_checksum()
  }

  /// Writes the header to a writer.
  pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
    writer.write_all(&self.stable_id.to_le_bytes())?;
    writer.write_all(&self.position_x.to_le_bytes())?;
    writer.write_all(&self.position_y.to_le_bytes())?;
    writer.write_all(&self.rotation.to_le_bytes())?;
    writer.write_all(&self.linear_velocity_x.to_le_bytes())?;
    writer.write_all(&self.linear_velocity_y.to_le_bytes())?;
    writer.write_all(&self.angular_velocity.to_le_bytes())?;
    writer.write_all(&self.width.to_le_bytes())?;
    writer.write_all(&self.height.to_le_bytes())?;
    writer.write_all(&self.origin_x.to_le_bytes())?;
    writer.write_all(&self.origin_y.to_le_bytes())?;
    writer.write_all(&self.pixel_data_size.to_le_bytes())?;
    writer.write_all(&self.shape_mask_size.to_le_bytes())?;
    writer.write_all(&self.extension_data_size.to_le_bytes())?;
    writer.write_all(&[self.checksum])?;
    writer.write_all(&self._reserved)?;
    Ok(())
  }

  /// Reads a header from a reader.
  pub fn read_from<R: Read>(reader: &mut R) -> io::Result<Self> {
    let mut buf = [0u8; Self::SIZE];
    reader.read_exact(&mut buf)?;

    Ok(Self {
      stable_id: u64::from_le_bytes(buf[0..8].try_into().unwrap()),
      position_x: f32::from_le_bytes(buf[8..12].try_into().unwrap()),
      position_y: f32::from_le_bytes(buf[12..16].try_into().unwrap()),
      rotation: f32::from_le_bytes(buf[16..20].try_into().unwrap()),
      linear_velocity_x: f32::from_le_bytes(buf[20..24].try_into().unwrap()),
      linear_velocity_y: f32::from_le_bytes(buf[24..28].try_into().unwrap()),
      angular_velocity: f32::from_le_bytes(buf[28..32].try_into().unwrap()),
      width: u32::from_le_bytes(buf[32..36].try_into().unwrap()),
      height: u32::from_le_bytes(buf[36..40].try_into().unwrap()),
      origin_x: i32::from_le_bytes(buf[40..44].try_into().unwrap()),
      origin_y: i32::from_le_bytes(buf[44..48].try_into().unwrap()),
      pixel_data_size: u32::from_le_bytes(buf[48..52].try_into().unwrap()),
      shape_mask_size: u32::from_le_bytes(buf[52..56].try_into().unwrap()),
      extension_data_size: u32::from_le_bytes(buf[56..60].try_into().unwrap()),
      checksum: buf[60],
      _reserved: [buf[61], buf[62], buf[63]],
    })
  }

  /// Returns the total size of variable data following this header.
  pub fn variable_data_size(&self) -> usize {
    self.pixel_data_size as usize
      + self.shape_mask_size as usize
      + self.extension_data_size as usize
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn header_round_trip() {
    let header = Header::new(12345);
    let mut buf = Vec::new();
    header.write_to(&mut buf).unwrap();
    assert_eq!(buf.len(), Header::SIZE);

    let mut cursor = std::io::Cursor::new(&buf);
    let read_header = Header::read_from(&mut cursor).unwrap();

    assert_eq!(read_header.magic, header.magic);
    assert_eq!(read_header.version, header.version);
    assert_eq!(read_header.world_seed, header.world_seed);
    assert_eq!(read_header.chunk_size, header.chunk_size);
  }

  #[test]
  fn page_table_entry_round_trip() {
    let pos = ChunkPos::new(-5, 10);
    let entry = PageTableEntry::new(pos, 1024, 512, StorageType::Delta);

    let mut buf = Vec::new();
    entry.write_to(&mut buf).unwrap();
    assert_eq!(buf.len(), PageTableEntry::SIZE);

    let mut cursor = std::io::Cursor::new(&buf);
    let read_entry = PageTableEntry::read_from(&mut cursor).unwrap();

    assert_eq!(read_entry.pos(), pos);
    assert_eq!(read_entry.data_offset, entry.data_offset);
    assert_eq!(read_entry.data_size, entry.data_size);
    assert_eq!(read_entry.storage_type, entry.storage_type);
    assert!(read_entry.validate_checksum());
  }

  #[test]
  fn checksum_detects_corruption() {
    let entry = PageTableEntry::new(ChunkPos::new(1, 2), 100, 50, StorageType::Full);
    assert!(entry.validate_checksum());

    let mut corrupted = entry;
    corrupted.chunk_x = 999;
    assert!(!corrupted.validate_checksum());
  }
}
