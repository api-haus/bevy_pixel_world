mod common;

use common::tile_processor::blit_with_tile_size;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use pixel_world::primitives::Surface;
use pixel_world::{ColorIndex, MaterialId, Pixel, Rgba};

const CHUNK_SIZES: &[u32] = &[256, 512, 1024];
const TILE_SIZES: &[u32] = &[8, 16, 32];

fn bench_blit(c: &mut Criterion) {
  let mut group = c.benchmark_group("blit");

  for &chunk_size in CHUNK_SIZES {
    let pixel_count = (chunk_size as u64) * (chunk_size as u64);
    group.throughput(Throughput::Elements(pixel_count));

    for &tile_size in TILE_SIZES {
      let id = BenchmarkId::new(format!("chunk_{chunk_size}"), format!("tile_{tile_size}"));

      group.bench_with_input(
        id,
        &(chunk_size, tile_size),
        |b, &(chunk_size, tile_size)| {
          let mut surface = Surface::<Pixel>::new(chunk_size, chunk_size);

          b.iter(|| {
            blit_with_tile_size(&mut surface, tile_size, |x, y| {
              // Simple checkerboard pattern as shader stand-in
              let mat = if (x + y) % 2 == 0 { 1 } else { 2 };
              Pixel::new(MaterialId(mat), ColorIndex(0))
            });
          });
        },
      );
    }
  }

  group.finish();
}

fn bench_upload(c: &mut Criterion) {
  let mut group = c.benchmark_group("upload");

  for &chunk_size in CHUNK_SIZES {
    let byte_count = (chunk_size as u64) * (chunk_size as u64) * 4; // RGBA = 4 bytes
    group.throughput(Throughput::Bytes(byte_count));

    let id = BenchmarkId::new("chunk", chunk_size);

    group.bench_with_input(id, &chunk_size, |b, &chunk_size| {
      let source = Surface::<Rgba>::new(chunk_size, chunk_size);
      let source_bytes = source.as_bytes();
      let mut dest = vec![0u8; source_bytes.len()];

      b.iter(|| {
        dest.copy_from_slice(source_bytes);
      });
    });
  }

  group.finish();
}

fn bench_parallel_scaling(c: &mut Criterion) {
  let mut group = c.benchmark_group("parallel_scaling");

  let chunk_size = 512u32;
  let tile_size = 16u32;
  let pixel_count = (chunk_size as u64) * (chunk_size as u64);
  group.throughput(Throughput::Elements(pixel_count));

  for threads in [1, 2, 4, 8] {
    let id = BenchmarkId::new("threads", threads);

    group.bench_with_input(id, &threads, |b, &threads| {
      let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap();

      let mut surface = Surface::<Pixel>::new(chunk_size, chunk_size);

      b.iter(|| {
        pool.install(|| {
          blit_with_tile_size(&mut surface, tile_size, |x, y| {
            let mat = if (x + y) % 2 == 0 { 1 } else { 2 };
            Pixel::new(MaterialId(mat), ColorIndex(0))
          });
        });
      });
    });
  }

  group.finish();
}

criterion_group!(benches, bench_blit, bench_upload, bench_parallel_scaling);
criterion_main!(benches);
