//! E2E test for chunk persistence layer.
//!
//! Tests save/load cycle:
//! 1. Create WorldSave
//! 2. Create chunk with painted pixels
//! 3. Save to disk via persistence API
//! 4. Load via seed_chunk_with_loaded (the async persistence pathway)
//! 5. Verify from_persistence flag and pixel data

use bevy_pixel_world::persistence::native::NativeFs;
use bevy_pixel_world::persistence::{LoadedChunk, compression, format::StorageType};
use bevy_pixel_world::{
  CHUNK_SIZE, Chunk, ChunkPos, ChunkSeeder, ColorIndex, Pixel, WorldSave, material_ids,
};
use tempfile::TempDir;

/// Minimal seeder that fills chunk with void.
/// Used as fallback when loading persisted chunks.
struct NoopSeeder;

impl ChunkSeeder for NoopSeeder {
  fn seed(&self, _pos: ChunkPos, chunk: &mut Chunk) {
    // Fill with void
    for y in 0..chunk.pixels.height() {
      for x in 0..chunk.pixels.width() {
        chunk.pixels[(x, y)] = Pixel::VOID;
      }
    }
  }
}

/// Seeds a chunk with optional pre-loaded persistence data.
///
/// This mirrors the logic in `streaming/seeding.rs::seed_chunk_with_loaded`.
fn seed_chunk_with_loaded(
  seeder: &dyn ChunkSeeder,
  pos: ChunkPos,
  loaded: Option<LoadedChunk>,
) -> Chunk {
  let mut chunk = Chunk::new(CHUNK_SIZE, CHUNK_SIZE);
  chunk.set_pos(pos);

  if let Some(loaded_chunk) = loaded {
    if loaded_chunk.seeder_needed {
      seeder.seed(pos, &mut chunk);
    }
    if loaded_chunk.apply_to(&mut chunk).is_ok() {
      chunk.from_persistence = true;
    } else {
      seeder.seed(pos, &mut chunk);
    }
  } else {
    seeder.seed(pos, &mut chunk);
  }

  chunk
}

#[test]
fn chunk_roundtrip_preserves_painted_pixels() {
  // 1. Create temp save file
  let temp_dir = TempDir::new().expect("Failed to create temp dir");
  let fs = NativeFs::new(temp_dir.path().to_path_buf()).unwrap();

  // 2. Create WorldSave and a chunk with painted pixels
  let mut save = WorldSave::create(&fs, "test.save", 42).expect("Failed to create save");

  let mut chunk = Chunk::new(CHUNK_SIZE, CHUNK_SIZE);
  chunk.set_pos(ChunkPos::new(0, 0));

  // 3. Paint pixels - 20x20 block of sand
  let paint_material = material_ids::SAND;
  for y in 100..120 {
    for x in 100..120 {
      chunk.pixels[(x, y)] = Pixel::new(paint_material, ColorIndex(200));
    }
  }

  // Verify painted before save
  assert_eq!(chunk.pixels[(110, 110)].material, paint_material);
  assert!(!chunk.from_persistence);

  // 4. Save chunk to disk
  let seeder = NoopSeeder;
  save
    .save_chunk(&chunk, ChunkPos::new(0, 0), &seeder)
    .expect("Failed to save chunk");
  save.flush().expect("Failed to flush save");

  // 5. Load chunk data (simulating what dispatch_chunk_loads does)
  let loaded = save.load_chunk(ChunkPos::new(0, 0), &seeder);

  // 6. Seed a fresh chunk using the loaded data
  let loaded_chunk = seed_chunk_with_loaded(&seeder, ChunkPos::new(0, 0), loaded);

  // 7. Verify from_persistence flag is set
  assert!(
    loaded_chunk.from_persistence,
    "Chunk loaded from disk should have from_persistence = true"
  );

  // 8. Verify painted pixels are restored
  assert_eq!(
    loaded_chunk.pixels[(110, 110)].material,
    paint_material,
    "Center of painted blob should have correct material"
  );

  // Check corners of the painted region
  assert_eq!(loaded_chunk.pixels[(100, 100)].material, paint_material);
  assert_eq!(loaded_chunk.pixels[(119, 119)].material, paint_material);
  assert_eq!(loaded_chunk.pixels[(100, 119)].material, paint_material);
  assert_eq!(loaded_chunk.pixels[(119, 100)].material, paint_material);

  // Check outside the painted region is void (from NoopSeeder fallback in delta,
  // or original void in full storage)
  assert_eq!(loaded_chunk.pixels[(99, 99)].material.0, 0);
  assert_eq!(loaded_chunk.pixels[(120, 120)].material.0, 0);
}

#[test]
fn unpersisted_chunk_uses_fallback_seeder() {
  // Create save file with no chunks
  let temp_dir = TempDir::new().expect("Failed to create temp dir");
  let fs = NativeFs::new(temp_dir.path().to_path_buf()).unwrap();

  let _save = WorldSave::create(&fs, "empty.save", 42).expect("Failed to create save");

  // Seeder that fills with stone
  struct StoneFiller;
  impl ChunkSeeder for StoneFiller {
    fn seed(&self, _pos: ChunkPos, chunk: &mut Chunk) {
      for y in 0..chunk.pixels.height() {
        for x in 0..chunk.pixels.width() {
          chunk.pixels[(x, y)] = Pixel::new(material_ids::STONE, ColorIndex(128));
        }
      }
    }
  }

  // Seed a chunk with no loaded data (simulating a chunk not in persistence)
  let chunk = seed_chunk_with_loaded(&StoneFiller, ChunkPos::new(99, 99), None);

  // Should NOT have from_persistence flag (procedurally generated)
  assert!(
    !chunk.from_persistence,
    "Procedurally generated chunk should have from_persistence = false"
  );

  // Should have stone from fallback seeder
  assert_eq!(chunk.pixels[(256, 256)].material, material_ids::STONE);
}

#[test]
fn loaded_chunk_data_applies_correctly() {
  // Test that LoadedChunk applies data correctly

  // Create compressed chunk data
  let mut pixels = vec![Pixel::VOID; (CHUNK_SIZE * CHUNK_SIZE) as usize];
  for y in 50..60 {
    for x in 50..60 {
      pixels[y * CHUNK_SIZE as usize + x] = Pixel::new(material_ids::WATER, ColorIndex(100));
    }
  }

  // Get bytes and compress
  let bytes: Vec<u8> = pixels
    .iter()
    .flat_map(|p| [p.material.0, p.color.0, p.damage, p.flags.bits()])
    .collect();
  let compressed = compression::compress_lz4(&bytes);

  // Create LoadedChunk
  let loaded = LoadedChunk {
    storage_type: StorageType::Full,
    data: compressed,
    pos: ChunkPos::new(0, 0),
    seeder_needed: false,
  };

  // Apply to a fresh chunk
  let chunk = seed_chunk_with_loaded(&NoopSeeder, ChunkPos::new(0, 0), Some(loaded));

  // Verify data
  assert!(chunk.from_persistence);
  assert_eq!(chunk.pixels[(55, 55)].material, material_ids::WATER);
  assert_eq!(chunk.pixels[(0, 0)].material.0, 0); // Void
}
