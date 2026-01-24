//! E2E test for chunk persistence layer.
//!
//! Tests save/load cycle directly without full Bevy ECS:
//! 1. Create WorldSave
//! 2. Create chunk with painted pixels
//! 3. Save to disk via persistence API
//! 4. Load via PersistenceSeeder
//! 5. Verify from_persistence flag and pixel data

use std::sync::{Arc, RwLock};

use tempfile::TempDir;

use bevy_pixel_world::{
    material_ids, Chunk, ChunkPos, ChunkSeeder, ColorIndex, PersistenceSeeder, Pixel, WorldSave,
    CHUNK_SIZE,
};

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

#[test]
fn chunk_roundtrip_preserves_painted_pixels() {
    // 1. Create temp save file
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let save_path = temp_dir.path().join("test.save");

    // 2. Create WorldSave and a chunk with painted pixels
    let mut save = WorldSave::create(&save_path, 42).expect("Failed to create save");

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
    save.save_chunk(&chunk, ChunkPos::new(0, 0), &seeder)
        .expect("Failed to save chunk");
    save.flush().expect("Failed to flush save");

    // 5. Create PersistenceSeeder with the save file
    let save_arc = Arc::new(RwLock::new(save));
    let persistent_seeder = PersistenceSeeder::new(NoopSeeder, save_arc);

    // 6. Seed a fresh chunk from the save file
    let mut loaded_chunk = Chunk::new(CHUNK_SIZE, CHUNK_SIZE);
    persistent_seeder.seed(ChunkPos::new(0, 0), &mut loaded_chunk);

    // 7. Verify from_persistence flag is set
    assert!(
        loaded_chunk.from_persistence,
        "Chunk loaded from disk should have from_persistence = true"
    );

    // 8. Verify painted pixels are restored
    assert_eq!(
        loaded_chunk.pixels[(110, 110)].material, paint_material,
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
    let save_path = temp_dir.path().join("empty.save");

    let save = WorldSave::create(&save_path, 42).expect("Failed to create save");
    let save_arc = Arc::new(RwLock::new(save));

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

    let persistent_seeder = PersistenceSeeder::new(StoneFiller, save_arc);

    // Seed a chunk that doesn't exist in persistence
    let mut chunk = Chunk::new(CHUNK_SIZE, CHUNK_SIZE);
    persistent_seeder.seed(ChunkPos::new(99, 99), &mut chunk);

    // Should NOT have from_persistence flag (procedurally generated)
    assert!(
        !chunk.from_persistence,
        "Procedurally generated chunk should have from_persistence = false"
    );

    // Should have stone from fallback seeder
    assert_eq!(chunk.pixels[(256, 256)].material, material_ids::STONE);
}
