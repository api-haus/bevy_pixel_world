//! Runtime index for chunk persistence.
//!
//! Maintains a HashMap for O(1) chunk lookups at runtime.
//! Serializes to sorted array on disk for forward scanning and recovery.

use std::collections::HashMap;
use std::io::{self, Read, Write};

use crate::coords::ChunkPos;

use super::format::PageTableEntry;

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
