//! Benchmarks for chunk persistence to identify data format boundaries.
//!
//! Key questions this answers:
//! - What compression ratios can we expect for different chunk patterns?
//! - Can compressed chunks fit in u16 (64KB) or do we need u32?
//! - At what scale does binary search become slow enough to warrant hierarchical indexing?
//! - How does delta compression compare to full compression?

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use lz4_flex::{compress_prepend_size, decompress_size_prepended};
use pixel_world::primitives::Surface;
use pixel_world::{ColorIndex, MaterialId, Pixel};
use rand::prelude::*;
use std::collections::HashMap;

const CHUNK_SIZE: u32 = 512;
const PIXELS_PER_CHUNK: usize = (CHUNK_SIZE * CHUNK_SIZE) as usize;
const BYTES_PER_CHUNK: usize = PIXELS_PER_CHUNK * 4;

// ============================================================================
// Data Generation Helpers
// ============================================================================

/// All air - best case for compression
fn generate_empty_chunk() -> Surface<Pixel> {
    Surface::<Pixel>::new(CHUNK_SIZE, CHUNK_SIZE)
}

/// Single material - very compressible
fn generate_uniform_chunk(material: u8) -> Surface<Pixel> {
    let mut surface = Surface::<Pixel>::new(CHUNK_SIZE, CHUNK_SIZE);
    for y in 0..CHUNK_SIZE {
        for x in 0..CHUNK_SIZE {
            surface.set(x, y, Pixel::new(MaterialId(material), ColorIndex(0)));
        }
    }
    surface
}

/// Random materials - worst case for compression
fn generate_random_chunk(seed: u64) -> Surface<Pixel> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut surface = Surface::<Pixel>::new(CHUNK_SIZE, CHUNK_SIZE);
    for y in 0..CHUNK_SIZE {
        for x in 0..CHUNK_SIZE {
            let pixel = Pixel {
                material: MaterialId(rng.gen_range(0..=255)),
                color: ColorIndex(rng.gen_range(0..=255)),
                damage: rng.gen_range(0..=255),
                flags: rng.gen_range(0..=255),
            };
            surface.set(x, y, pixel);
        }
    }
    surface
}

/// Layered terrain - realistic game scenario
fn generate_terrain_chunk(seed: u64) -> Surface<Pixel> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut surface = Surface::<Pixel>::new(CHUNK_SIZE, CHUNK_SIZE);

    // Ground level with some noise
    let base_ground = CHUNK_SIZE / 2;

    for x in 0..CHUNK_SIZE {
        // Wavy ground line
        let noise = (rng.gen_range(0.0..1.0f32) * 20.0) as u32;
        let ground = base_ground + noise;

        for y in 0..CHUNK_SIZE {
            let pixel = if y < ground.saturating_sub(50) {
                // Deep stone
                Pixel::new(MaterialId(3), ColorIndex(rng.gen_range(0..4)))
            } else if y < ground.saturating_sub(10) {
                // Dirt
                Pixel::new(MaterialId(2), ColorIndex(rng.gen_range(0..4)))
            } else if y < ground {
                // Grass/topsoil
                Pixel::new(MaterialId(1), ColorIndex(rng.gen_range(0..2)))
            } else {
                // Air
                Pixel::AIR
            };
            surface.set(x, y, pixel);
        }
    }
    surface
}

/// Sparse modifications on empty - delta compression scenario
fn generate_sparse_chunk(seed: u64, density: f32) -> Surface<Pixel> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut surface = Surface::<Pixel>::new(CHUNK_SIZE, CHUNK_SIZE);

    let num_modifications = (PIXELS_PER_CHUNK as f32 * density) as usize;

    for _ in 0..num_modifications {
        let x = rng.gen_range(0..CHUNK_SIZE);
        let y = rng.gen_range(0..CHUNK_SIZE);
        surface.set(x, y, Pixel::new(MaterialId(1), ColorIndex(0)));
    }
    surface
}

// ============================================================================
// Compression Benchmarks
// ============================================================================

fn bench_compression_ratios(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression_ratio");
    group.throughput(Throughput::Bytes(BYTES_PER_CHUNK as u64));

    let scenarios: Vec<(&str, Surface<Pixel>)> = vec![
        ("empty", generate_empty_chunk()),
        ("uniform", generate_uniform_chunk(1)),
        ("random", generate_random_chunk(42)),
        ("terrain", generate_terrain_chunk(42)),
        ("sparse_1pct", generate_sparse_chunk(42, 0.01)),
        ("sparse_5pct", generate_sparse_chunk(42, 0.05)),
        ("sparse_25pct", generate_sparse_chunk(42, 0.25)),
    ];

    // First, print compression statistics (outside of timed benchmark)
    println!("\n=== Compression Statistics (512x512 chunks, 1MB uncompressed) ===\n");
    println!(
        "{:<15} {:>12} {:>12} {:>8}",
        "Pattern", "Compressed", "Ratio", "Fits u16?"
    );
    println!("{}", "-".repeat(50));

    for (name, surface) in &scenarios {
        let bytes = surface.as_bytes();
        let compressed = compress_prepend_size(bytes);
        let ratio = bytes.len() as f64 / compressed.len() as f64;
        let fits_u16 = compressed.len() <= 65535;

        println!(
            "{:<15} {:>10} B {:>10.1}x {:>8}",
            name,
            compressed.len(),
            ratio,
            if fits_u16 { "YES" } else { "NO" }
        );
    }
    println!();

    // Now benchmark compression speed
    for (name, surface) in scenarios {
        let bytes = surface.as_bytes();

        group.bench_with_input(BenchmarkId::new("compress", name), bytes, |b, bytes| {
            b.iter(|| compress_prepend_size(black_box(bytes)))
        });
    }

    group.finish();
}

fn bench_decompression(c: &mut Criterion) {
    let mut group = c.benchmark_group("decompression");
    group.throughput(Throughput::Bytes(BYTES_PER_CHUNK as u64));

    let scenarios: Vec<(&str, Vec<u8>)> = vec![
        ("empty", compress_prepend_size(generate_empty_chunk().as_bytes())),
        ("uniform", compress_prepend_size(generate_uniform_chunk(1).as_bytes())),
        ("random", compress_prepend_size(generate_random_chunk(42).as_bytes())),
        ("terrain", compress_prepend_size(generate_terrain_chunk(42).as_bytes())),
    ];

    for (name, compressed) in scenarios {
        group.bench_with_input(
            BenchmarkId::new("decompress", name),
            &compressed,
            |b, compressed| b.iter(|| decompress_size_prepended(black_box(compressed)).unwrap()),
        );
    }

    group.finish();
}

// ============================================================================
// Page Table Lookup Benchmarks
// ============================================================================

/// Simulates page table entry
#[derive(Clone, Copy)]
struct PageTableEntry {
    chunk_x: i32,
    chunk_y: i32,
    data_offset: u32,
    data_size: u16,
}

fn generate_page_table(num_entries: usize, seed: u64) -> Vec<PageTableEntry> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut entries: Vec<PageTableEntry> = (0..num_entries)
        .map(|_| PageTableEntry {
            chunk_x: rng.gen_range(-100_000..100_000),
            chunk_y: rng.gen_range(-100_000..100_000),
            data_offset: rng.next_u32(),
            data_size: rng.gen_range(0..=u16::MAX),
        })
        .collect();

    // Sort by (y, x) as specified in the design
    entries.sort_by_key(|e| (e.chunk_y, e.chunk_x));
    entries
}

fn binary_search_lookup(table: &[PageTableEntry], x: i32, y: i32) -> Option<&PageTableEntry> {
    table
        .binary_search_by_key(&(y, x), |e| (e.chunk_y, e.chunk_x))
        .ok()
        .map(|idx| &table[idx])
}

fn bench_page_table_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("page_table_lookup");

    let sizes = [1_000, 10_000, 100_000, 1_000_000];

    println!("\n=== Page Table Memory Usage ===\n");
    println!("{:<12} {:>15} {:>15}", "Entries", "Size (bytes)", "Size (human)");
    println!("{}", "-".repeat(45));

    for &size in &sizes {
        let bytes = size * std::mem::size_of::<PageTableEntry>();
        let human = if bytes >= 1_000_000 {
            format!("{:.1} MB", bytes as f64 / 1_000_000.0)
        } else {
            format!("{:.1} KB", bytes as f64 / 1_000.0)
        };
        println!("{:<12} {:>15} {:>15}", size, bytes, human);
    }
    println!();

    for &size in &sizes {
        let table = generate_page_table(size, 42);
        let mut rng = StdRng::seed_from_u64(123);

        // Generate lookup targets (mix of hits and misses)
        let targets: Vec<(i32, i32)> = (0..1000)
            .map(|i| {
                if i % 2 == 0 && !table.is_empty() {
                    // Hit: use existing entry
                    let entry = &table[rng.gen_range(0..table.len())];
                    (entry.chunk_x, entry.chunk_y)
                } else {
                    // Miss: random coords
                    (rng.gen_range(-100_000..100_000), rng.gen_range(-100_000..100_000))
                }
            })
            .collect();

        group.throughput(Throughput::Elements(1000));
        group.bench_with_input(
            BenchmarkId::new("binary_search", size),
            &(&table, &targets),
            |b, &(table, targets)| {
                b.iter(|| {
                    for &(x, y) in targets {
                        black_box(binary_search_lookup(table, x, y));
                    }
                })
            },
        );
    }

    // Compare with HashMap lookup
    for &size in &sizes {
        let table = generate_page_table(size, 42);
        let hash_map: HashMap<(i32, i32), PageTableEntry> = table
            .iter()
            .map(|e| ((e.chunk_x, e.chunk_y), *e))
            .collect();

        let mut rng = StdRng::seed_from_u64(123);
        let targets: Vec<(i32, i32)> = (0..1000)
            .map(|i| {
                if i % 2 == 0 && !table.is_empty() {
                    let entry = &table[rng.gen_range(0..table.len())];
                    (entry.chunk_x, entry.chunk_y)
                } else {
                    (rng.gen_range(-100_000..100_000), rng.gen_range(-100_000..100_000))
                }
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::new("hash_map", size),
            &(&hash_map, &targets),
            |b, &(map, targets)| {
                b.iter(|| {
                    for &key in targets {
                        black_box(map.get(&key));
                    }
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// Delta Compression Benchmarks
// ============================================================================

/// Compute delta between two chunks
fn compute_delta(original: &Surface<Pixel>, modified: &Surface<Pixel>) -> Vec<(u32, Pixel)> {
    let mut deltas = Vec::new();
    let orig_bytes = original.as_bytes();
    let mod_bytes = modified.as_bytes();

    for i in 0..PIXELS_PER_CHUNK {
        let orig_pixel = &orig_bytes[i * 4..(i + 1) * 4];
        let mod_pixel = &mod_bytes[i * 4..(i + 1) * 4];
        if orig_pixel != mod_pixel {
            deltas.push((
                i as u32,
                Pixel {
                    material: MaterialId(mod_pixel[0]),
                    color: ColorIndex(mod_pixel[1]),
                    damage: mod_pixel[2],
                    flags: mod_pixel[3],
                },
            ));
        }
    }
    deltas
}

/// Generate a modified terrain chunk (base terrain + random modifications)
fn generate_modified_terrain(base_seed: u64, mod_seed: u64, density: f32) -> Surface<Pixel> {
    let mut surface = generate_terrain_chunk(base_seed);
    let mut rng = StdRng::seed_from_u64(mod_seed);
    let num_mods = (PIXELS_PER_CHUNK as f32 * density) as usize;

    for _ in 0..num_mods {
        let x = rng.gen_range(0..CHUNK_SIZE);
        let y = rng.gen_range(0..CHUNK_SIZE);
        surface.set(x, y, Pixel::new(MaterialId(5), ColorIndex(0)));
    }
    surface
}

fn bench_delta_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("delta_compression");

    let densities = [0.01, 0.05, 0.10, 0.25, 0.50];

    println!("\n=== Delta vs Full Compression (terrain base) ===\n");
    println!(
        "{:<12} {:>12} {:>12} {:>12} {:>10}",
        "Density", "Delta Size", "Full Size", "Delta Ents", "Winner"
    );
    println!("{}", "-".repeat(60));

    // Pre-generate all surfaces for analysis
    let base = generate_terrain_chunk(42);
    let modified_chunks: Vec<_> = densities
        .iter()
        .map(|&d| generate_modified_terrain(42, 123, d))
        .collect();

    for (i, &density) in densities.iter().enumerate() {
        let modified = &modified_chunks[i];
        let deltas = compute_delta(&base, modified);

        // Delta format: 4 bytes count + (3 bytes pos + 4 bytes pixel) per entry
        let delta_uncompressed = 4 + deltas.len() * 7;
        let delta_compressed = compress_prepend_size(&vec![0u8; delta_uncompressed]).len();

        let full_compressed = compress_prepend_size(modified.as_bytes()).len();

        let winner = if delta_compressed < full_compressed {
            "Delta"
        } else {
            "Full"
        };

        println!(
            "{:<12} {:>10} B {:>10} B {:>12} {:>10}",
            format!("{:.0}%", density * 100.0),
            delta_compressed,
            full_compressed,
            deltas.len(),
            winner
        );
    }
    println!();

    // Benchmark delta computation with pre-generated surfaces
    for (i, &density) in densities.iter().enumerate() {
        let modified = &modified_chunks[i];
        group.bench_with_input(
            BenchmarkId::new("compute_delta", format!("{:.0}pct", density * 100.0)),
            &(&base, modified),
            |b, &(base, modified)| {
                b.iter(|| compute_delta(black_box(base), black_box(modified)))
            },
        );
    }

    group.finish();
}

// ============================================================================
// World Size Limits
// ============================================================================

fn print_format_limits() {
    println!("\n=== File Format Limits Analysis ===\n");

    // Current format limits
    let data_offset_max: u64 = u32::MAX as u64; // 4GB
    let page_table_entry_size: u64 = 16;
    let avg_compressed_chunk: u64 = 50_000; // ~50KB for terrain (from benchmarks)

    let max_chunks_by_data = data_offset_max / avg_compressed_chunk;
    let _max_chunks_by_table = data_offset_max / page_table_entry_size;

    println!("Current format (u32 Data Offset):");
    println!("  Max data region:     {} GB", data_offset_max / 1_000_000_000);
    println!(
        "  Max chunks (50KB avg): {} (~{:.1}M)",
        max_chunks_by_data,
        max_chunks_by_data as f64 / 1_000_000.0
    );
    println!(
        "  World size (square):   {}x{} chunks",
        (max_chunks_by_data as f64).sqrt() as u64,
        (max_chunks_by_data as f64).sqrt() as u64
    );
    println!(
        "  World size (pixels):   {}x{} ({:.0} km at 1px=1cm)",
        (max_chunks_by_data as f64).sqrt() as u64 * 512,
        (max_chunks_by_data as f64).sqrt() as u64 * 512,
        (max_chunks_by_data as f64).sqrt() * 512.0 / 100_000.0
    );

    println!("\nWith u64 Data Offset:");
    let data_offset_max_64: u64 = u64::MAX;
    let max_chunks_64 = data_offset_max_64 / avg_compressed_chunk;
    println!(
        "  Max chunks:           {} (effectively unlimited)",
        if max_chunks_64 > 1_000_000_000_000 {
            ">1 trillion".to_string()
        } else {
            format!("{}", max_chunks_64)
        }
    );

    println!("\n=== Field Size Recommendations ===\n");
    println!("Data Size field (per-chunk compressed size):");
    println!("  u16 (64KB):  INSUFFICIENT for 512x512 chunks");
    println!("  u24 (16MB):  Safe margin");
    println!("  u32 (4GB):   Overkill but simple");

    println!("\nData Offset field (file position):");
    println!("  u32 (4GB):   ~80K chunks at 50KB avg");
    println!("  u48 (256TB): Effectively unlimited");
    println!("  u64 (16EB):  Maximum safety");
}

fn bench_world_limits(_c: &mut Criterion) {
    // This isn't really a benchmark, just prints analysis
    print_format_limits();
}

criterion_group!(
    benches,
    bench_compression_ratios,
    bench_decompression,
    bench_page_table_lookup,
    bench_delta_compression,
    bench_world_limits,
);
criterion_main!(benches);
