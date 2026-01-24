//! Benchmarks for collision mesh generation pipeline.
//!
//! Tests each stage of the collision pipeline across different grid patterns,
//! from simplest (empty) to most complex (random noise).

use bevy::math::Vec2;
use bevy_pixel_world::collision::{
    marching_squares, simplify_polylines, triangulate_polygons, GRID_SIZE,
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::prelude::*;

// ============================================================================
// Grid Generators
// ============================================================================

/// All cells empty - best case, early exit.
fn empty_grid() -> [[bool; GRID_SIZE]; GRID_SIZE] {
    [[false; GRID_SIZE]; GRID_SIZE]
}

/// Single filled rectangle.
fn solid_block_grid(x: usize, y: usize, w: usize, h: usize) -> [[bool; GRID_SIZE]; GRID_SIZE] {
    let mut grid = [[false; GRID_SIZE]; GRID_SIZE];
    for row in y..(y + h).min(GRID_SIZE) {
        for col in x..(x + w).min(GRID_SIZE) {
            grid[row][col] = true;
        }
    }
    grid
}

/// Entire tile solid (border will be forced empty by marching squares).
fn full_tile_grid() -> [[bool; GRID_SIZE]; GRID_SIZE] {
    [[true; GRID_SIZE]; GRID_SIZE]
}

/// Centered filled circle.
fn circle_grid(radius: f32) -> [[bool; GRID_SIZE]; GRID_SIZE] {
    let mut grid = [[false; GRID_SIZE]; GRID_SIZE];
    let center = GRID_SIZE as f32 / 2.0;
    let radius_sq = radius * radius;

    for y in 0..GRID_SIZE {
        for x in 0..GRID_SIZE {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            if dx * dx + dy * dy <= radius_sq {
                grid[y][x] = true;
            }
        }
    }
    grid
}

/// Terrain-like pattern: bottom half solid with noise variation.
fn terrain_grid(seed: u64) -> [[bool; GRID_SIZE]; GRID_SIZE] {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut grid = [[false; GRID_SIZE]; GRID_SIZE];

    // Base terrain height with per-column variation
    for x in 0..GRID_SIZE {
        let base_height = GRID_SIZE / 2;
        let variation = rng.gen_range(-4..=4i32);
        let height = (base_height as i32 + variation).clamp(0, GRID_SIZE as i32 - 1) as usize;

        for y in 0..height {
            grid[y][x] = true;
        }
    }
    grid
}

/// Multiple disconnected blobs.
fn islands_grid(count: usize, seed: u64) -> [[bool; GRID_SIZE]; GRID_SIZE] {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut grid = [[false; GRID_SIZE]; GRID_SIZE];

    for _ in 0..count {
        // Random center and radius for each island
        let cx = rng.gen_range(4..GRID_SIZE - 4);
        let cy = rng.gen_range(4..GRID_SIZE - 4);
        let radius = rng.gen_range(2..6) as f32;
        let radius_sq = radius * radius;

        for y in 0..GRID_SIZE {
            for x in 0..GRID_SIZE {
                let dx = x as f32 - cx as f32;
                let dy = y as f32 - cy as f32;
                if dx * dx + dy * dy <= radius_sq {
                    grid[y][x] = true;
                }
            }
        }
    }
    grid
}

/// Terrain with interior holes (caves).
fn caves_grid(seed: u64) -> [[bool; GRID_SIZE]; GRID_SIZE] {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut grid = terrain_grid(seed);

    // Carve out some caves
    let num_caves = 3;
    for _ in 0..num_caves {
        let cx = rng.gen_range(6..GRID_SIZE - 6);
        let cy = rng.gen_range(4..GRID_SIZE / 2 - 2);
        let radius = rng.gen_range(2..5) as f32;
        let radius_sq = radius * radius;

        for y in 0..GRID_SIZE {
            for x in 0..GRID_SIZE {
                let dx = x as f32 - cx as f32;
                let dy = y as f32 - cy as f32;
                if dx * dx + dy * dy <= radius_sq {
                    grid[y][x] = false;
                }
            }
        }
    }
    grid
}

/// Random fill at given density - worst case for contour complexity.
fn noise_grid(density: f32, seed: u64) -> [[bool; GRID_SIZE]; GRID_SIZE] {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut grid = [[false; GRID_SIZE]; GRID_SIZE];

    for y in 0..GRID_SIZE {
        for x in 0..GRID_SIZE {
            grid[y][x] = rng.gen_bool(density as f64);
        }
    }
    grid
}

// ============================================================================
// Test Cases
// ============================================================================

struct TestCase {
    name: &'static str,
    grid: [[bool; GRID_SIZE]; GRID_SIZE],
}

fn test_cases() -> Vec<TestCase> {
    vec![
        TestCase {
            name: "empty",
            grid: empty_grid(),
        },
        TestCase {
            name: "solid_block",
            grid: solid_block_grid(10, 10, 14, 14),
        },
        TestCase {
            name: "full_tile",
            grid: full_tile_grid(),
        },
        TestCase {
            name: "circle",
            grid: circle_grid(12.0),
        },
        TestCase {
            name: "terrain",
            grid: terrain_grid(42),
        },
        TestCase {
            name: "islands",
            grid: islands_grid(5, 42),
        },
        TestCase {
            name: "caves",
            grid: caves_grid(42),
        },
        TestCase {
            name: "noise_50pct",
            grid: noise_grid(0.5, 42),
        },
    ]
}

// ============================================================================
// Benchmark: Marching Squares
// ============================================================================

fn bench_marching_squares(c: &mut Criterion) {
    let mut group = c.benchmark_group("collision/marching_squares");
    group.throughput(Throughput::Elements(1)); // 1 tile per iteration

    for case in test_cases() {
        group.bench_with_input(BenchmarkId::new("extract", case.name), &case.grid, |b, grid| {
            b.iter(|| marching_squares(black_box(grid), Vec2::ZERO))
        });
    }

    group.finish();
}

// ============================================================================
// Benchmark: Simplification
// ============================================================================

fn bench_simplification(c: &mut Criterion) {
    let mut group = c.benchmark_group("collision/simplification");

    // Pre-generate polylines for each case
    let cases_with_polylines: Vec<_> = test_cases()
        .into_iter()
        .map(|case| {
            let polylines = marching_squares(&case.grid, Vec2::ZERO);
            let vertex_count: usize = polylines.iter().map(|p| p.len()).sum();
            (case.name, polylines, vertex_count)
        })
        .collect();

    for (name, polylines, vertex_count) in &cases_with_polylines {
        if *vertex_count == 0 {
            continue; // Skip empty cases
        }

        group.throughput(Throughput::Elements(*vertex_count as u64));
        group.bench_with_input(
            BenchmarkId::new("douglas_peucker", name),
            polylines,
            |b, polylines| {
                b.iter(|| simplify_polylines(black_box(polylines.clone()), 1.0))
            },
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark: Triangulation
// ============================================================================

fn bench_triangulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("collision/triangulation");

    // Pre-generate simplified polylines for each case
    let cases_with_polygons: Vec<_> = test_cases()
        .into_iter()
        .map(|case| {
            let polylines = marching_squares(&case.grid, Vec2::ZERO);
            let simplified = simplify_polylines(polylines, 1.0);
            let vertex_count: usize = simplified.iter().map(|p| p.len()).sum();
            (case.name, simplified, vertex_count)
        })
        .collect();

    for (name, polygons, vertex_count) in &cases_with_polygons {
        if *vertex_count == 0 {
            continue;
        }

        group.throughput(Throughput::Elements(*vertex_count as u64));
        group.bench_with_input(BenchmarkId::new("cdt", name), polygons, |b, polygons| {
            b.iter(|| triangulate_polygons(black_box(polygons)))
        });
    }

    group.finish();
}

// ============================================================================
// Benchmark: Full Pipeline
// ============================================================================

fn bench_full_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("collision/full_pipeline");
    group.throughput(Throughput::Elements(1)); // 1 tile per iteration

    for case in test_cases() {
        group.bench_with_input(BenchmarkId::new("tile", case.name), &case.grid, |b, grid| {
            b.iter(|| {
                let polylines = marching_squares(black_box(grid), Vec2::ZERO);
                let simplified = simplify_polylines(polylines, 1.0);
                triangulate_polygons(&simplified)
            })
        });
    }

    group.finish();
}

// ============================================================================
// Statistics Output
// ============================================================================

fn print_pipeline_stats(_c: &mut Criterion) {
    println!("\n=== Collision Pipeline Statistics ===\n");
    println!(
        "{:<15} {:>10} {:>12} {:>12} {:>12}",
        "Case", "Contours", "Raw Verts", "Simplified", "Triangles"
    );
    println!("{}", "-".repeat(65));

    for case in test_cases() {
        let polylines = marching_squares(&case.grid, Vec2::ZERO);
        let raw_vertices: usize = polylines.iter().map(|p| p.len()).sum();

        let simplified = simplify_polylines(polylines.clone(), 1.0);
        let simplified_vertices: usize = simplified.iter().map(|p| p.len()).sum();

        let triangulated = triangulate_polygons(&simplified);
        let triangle_count: usize = triangulated.iter().map(|(_, t)| t.len()).sum();

        println!(
            "{:<15} {:>10} {:>12} {:>12} {:>12}",
            case.name,
            simplified.len(),
            raw_vertices,
            simplified_vertices,
            triangle_count
        );
    }
    println!();
}

criterion_group!(
    benches,
    print_pipeline_stats,
    bench_marching_squares,
    bench_simplification,
    bench_triangulation,
    bench_full_pipeline,
);
criterion_main!(benches);
