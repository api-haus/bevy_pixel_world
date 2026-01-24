//! Runtime index for chunk persistence.
//!
//! Maintains a HashMap for O(1) chunk lookups at runtime.
//! Serializes to sorted array on disk for forward scanning and recovery.

use std::collections::HashMap;
use std::io::{self, Read, Write};

use super::format::PageTableEntry;
use crate::coords::ChunkPos;

/// Runtime index for chunk positions to page table entries.
///
/// Uses HashMap for O(1) lookups. Serializes to sorted array on disk.
#[derive(Debug, Default)]
pub struct ChunkIndex {
  entries: HashMap<ChunkPos, PageTableEntry>,
}

impl ChunkIndex {
  /// Creates an empty index.
  pub fn new() -> Self {
    Self {
      entries: HashMap::new(),
    }
  }

  /// Creates an index with the given capacity.
  pub fn with_capacity(capacity: usize) -> Self {
    Self {
      entries: HashMap::with_capacity(capacity),
    }
  }

  /// Returns the number of entries.
  pub fn len(&self) -> usize {
    self.entries.len()
  }

  /// Returns true if the index is empty.
  pub fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }

  /// Looks up an entry by chunk position.
  pub fn get(&self, pos: ChunkPos) -> Option<&PageTableEntry> {
    self.entries.get(&pos)
  }

  /// Inserts or updates an entry.
  pub fn insert(&mut self, entry: PageTableEntry) {
    self.entries.insert(entry.pos(), entry);
  }

  /// Removes an entry by chunk position.
  pub fn remove(&mut self, pos: ChunkPos) -> Option<PageTableEntry> {
    self.entries.remove(&pos)
  }

  /// Returns true if the index contains the given position.
  pub fn contains(&self, pos: ChunkPos) -> bool {
    self.entries.contains_key(&pos)
  }

  /// Iterates over all entries.
  pub fn iter(&self) -> impl Iterator<Item = (&ChunkPos, &PageTableEntry)> {
    self.entries.iter()
  }

  /// Writes the index as a sorted array to a writer.
  ///
  /// Entries are sorted by (chunk_y, chunk_x) for spatial locality
  /// and forward scanning during recovery.
  pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
    // Collect and sort entries
    let mut entries: Vec<_> = self.entries.values().collect();
    entries.sort_by_key(|e| (e.chunk_y, e.chunk_x));

    // Write each entry
    for entry in entries {
      entry.write_to(writer)?;
    }

    Ok(())
  }

  /// Reads the index from a reader.
  ///
  /// Reads exactly `count` entries and builds the HashMap.
  pub fn read_from<R: Read>(reader: &mut R, count: usize) -> io::Result<Self> {
    let mut index = Self::with_capacity(count);

    for _ in 0..count {
      let entry = PageTableEntry::read_from(reader)?;

      // Validate checksum, skip corrupted entries
      if !entry.validate_checksum() {
        eprintln!(
          "Warning: skipping corrupted page table entry at ({}, {})",
          entry.chunk_x, entry.chunk_y
        );
        continue;
      }

      index.insert(entry);
    }

    Ok(index)
  }

  /// Returns the total serialized size in bytes.
  pub fn serialized_size(&self) -> usize {
    self.entries.len() * PageTableEntry::SIZE
  }

  /// Clears all entries.
  pub fn clear(&mut self) {
    self.entries.clear();
  }
}

/// Index entry for a persisted pixel body.
#[derive(Clone, Copy, Debug)]
pub struct PixelBodyIndexEntry {
  /// Stable ID of the pixel body.
  pub stable_id: u64,
  /// File offset to the PixelBodyRecordHeader.
  pub data_offset: u64,
  /// Total size of the record (header + variable data).
  pub data_size: u32,
  /// Chunk position where this body's center is located.
  pub chunk_pos: ChunkPos,
}

impl PixelBodyIndexEntry {
  /// Entry size in bytes for serialization.
  pub const SIZE: usize = 28;

  /// Writes this entry to a writer.
  pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
    writer.write_all(&self.stable_id.to_le_bytes())?;
    writer.write_all(&self.data_offset.to_le_bytes())?;
    writer.write_all(&self.data_size.to_le_bytes())?;
    writer.write_all(&self.chunk_pos.x.to_le_bytes())?;
    writer.write_all(&self.chunk_pos.y.to_le_bytes())?;
    Ok(())
  }

  /// Reads an entry from a reader.
  pub fn read_from<R: Read>(reader: &mut R) -> io::Result<Self> {
    let mut buf = [0u8; Self::SIZE];
    reader.read_exact(&mut buf)?;
    Ok(Self {
      stable_id: u64::from_le_bytes(buf[0..8].try_into().unwrap()),
      data_offset: u64::from_le_bytes(buf[8..16].try_into().unwrap()),
      data_size: u32::from_le_bytes(buf[16..20].try_into().unwrap()),
      chunk_pos: ChunkPos::new(
        i32::from_le_bytes(buf[20..24].try_into().unwrap()),
        i32::from_le_bytes(buf[24..28].try_into().unwrap()),
      ),
    })
  }
}

/// Runtime index for pixel bodies.
///
/// Maps chunk positions to the pixel bodies whose centers are in that chunk.
/// Also maintains a by-ID lookup for deduplication.
#[derive(Debug, Default)]
pub struct PixelBodyIndex {
  /// Bodies indexed by their stable ID.
  by_id: HashMap<u64, PixelBodyIndexEntry>,
  /// Bodies indexed by chunk position.
  by_chunk: HashMap<ChunkPos, Vec<u64>>,
}

impl PixelBodyIndex {
  /// Creates an empty index.
  pub fn new() -> Self {
    Self::default()
  }

  /// Returns the number of indexed bodies.
  pub fn len(&self) -> usize {
    self.by_id.len()
  }

  /// Returns true if the index is empty.
  pub fn is_empty(&self) -> bool {
    self.by_id.is_empty()
  }

  /// Looks up an entry by stable ID.
  pub fn get(&self, stable_id: u64) -> Option<&PixelBodyIndexEntry> {
    self.by_id.get(&stable_id)
  }

  /// Returns all bodies in a given chunk.
  pub fn get_chunk(&self, pos: ChunkPos) -> impl Iterator<Item = &PixelBodyIndexEntry> {
    self
      .by_chunk
      .get(&pos)
      .into_iter()
      .flat_map(|ids| ids.iter())
      .filter_map(|id| self.by_id.get(id))
  }

  /// Inserts or updates an entry.
  pub fn insert(&mut self, entry: PixelBodyIndexEntry) {
    // Remove from old chunk if exists
    if let Some(old) = self.by_id.get(&entry.stable_id) {
      if old.chunk_pos != entry.chunk_pos {
        if let Some(ids) = self.by_chunk.get_mut(&old.chunk_pos) {
          ids.retain(|&id| id != entry.stable_id);
        }
      }
    }

    // Add to new chunk
    self
      .by_chunk
      .entry(entry.chunk_pos)
      .or_default()
      .push(entry.stable_id);

    // Update by-ID index
    self.by_id.insert(entry.stable_id, entry);
  }

  /// Removes an entry by stable ID.
  pub fn remove(&mut self, stable_id: u64) -> Option<PixelBodyIndexEntry> {
    if let Some(entry) = self.by_id.remove(&stable_id) {
      if let Some(ids) = self.by_chunk.get_mut(&entry.chunk_pos) {
        ids.retain(|&id| id != stable_id);
      }
      Some(entry)
    } else {
      None
    }
  }

  /// Removes all bodies in a given chunk.
  pub fn remove_chunk(&mut self, pos: ChunkPos) -> Vec<PixelBodyIndexEntry> {
    let ids = self.by_chunk.remove(&pos).unwrap_or_default();
    ids
      .into_iter()
      .filter_map(|id| self.by_id.remove(&id))
      .collect()
  }

  /// Returns true if the index contains a body with the given ID.
  pub fn contains(&self, stable_id: u64) -> bool {
    self.by_id.contains_key(&stable_id)
  }

  /// Iterates over all entries.
  pub fn iter(&self) -> impl Iterator<Item = &PixelBodyIndexEntry> {
    self.by_id.values()
  }

  /// Writes the index to a writer.
  pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
    // Sort by chunk position for locality
    let mut entries: Vec<_> = self.by_id.values().collect();
    entries.sort_by_key(|e| (e.chunk_pos.y, e.chunk_pos.x, e.stable_id));

    for entry in entries {
      entry.write_to(writer)?;
    }

    Ok(())
  }

  /// Reads the index from a reader.
  pub fn read_from<R: Read>(reader: &mut R, count: usize) -> io::Result<Self> {
    let mut index = Self::new();

    for _ in 0..count {
      let entry = PixelBodyIndexEntry::read_from(reader)?;
      index.insert(entry);
    }

    Ok(index)
  }

  /// Returns the total serialized size in bytes.
  pub fn serialized_size(&self) -> usize {
    self.by_id.len() * PixelBodyIndexEntry::SIZE
  }

  /// Clears all entries.
  pub fn clear(&mut self) {
    self.by_id.clear();
    self.by_chunk.clear();
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::persistence::format::StorageType;

  #[test]
  fn index_insert_and_lookup() {
    let mut index = ChunkIndex::new();

    let pos1 = ChunkPos::new(0, 0);
    let pos2 = ChunkPos::new(-5, 10);

    let entry1 = PageTableEntry::new(pos1, 100, 50, StorageType::Full);
    let entry2 = PageTableEntry::new(pos2, 200, 75, StorageType::Delta);

    index.insert(entry1);
    index.insert(entry2);

    assert_eq!(index.len(), 2);
    assert!(index.contains(pos1));
    assert!(index.contains(pos2));

    let found = index.get(pos1).unwrap();
    assert_eq!(found.data_offset, 100);
  }

  #[test]
  fn index_round_trip() {
    let mut index = ChunkIndex::new();

    // Add entries in random order
    let entries = [
      PageTableEntry::new(ChunkPos::new(5, 10), 100, 50, StorageType::Full),
      PageTableEntry::new(ChunkPos::new(-3, 2), 200, 60, StorageType::Delta),
      PageTableEntry::new(ChunkPos::new(0, 0), 300, 70, StorageType::Empty),
      PageTableEntry::new(ChunkPos::new(1, -1), 400, 80, StorageType::Full),
    ];

    for entry in &entries {
      index.insert(*entry);
    }

    // Write to buffer
    let mut buf = Vec::new();
    index.write_to(&mut buf).unwrap();

    // Verify size
    assert_eq!(buf.len(), entries.len() * PageTableEntry::SIZE);

    // Read back
    let mut cursor = std::io::Cursor::new(&buf);
    let read_index = ChunkIndex::read_from(&mut cursor, entries.len()).unwrap();

    assert_eq!(read_index.len(), index.len());

    // Verify all entries present
    for entry in &entries {
      let found = read_index.get(entry.pos()).unwrap();
      assert_eq!(found.data_offset, entry.data_offset);
      assert_eq!(found.storage_type, entry.storage_type);
    }
  }
}
