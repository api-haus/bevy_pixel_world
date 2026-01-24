//! Benchmarks for deterministic hash functions.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use bevy_pixel_world::simulation::hash::*;

const ITERATIONS: u64 = 10_000;

/// Generate benchmarks for all hash function variants.
macro_rules! bench_hash_functions {
    ($group:expr; $($bits:tt: $uint:ty, $int:ty, $float:ty);+ $(;)?) => {
        $(
            bench_hash_functions!(@bits $group; $bits: $uint, $int, $float);
        )+
    };

    // Generate all input counts for a bit width
    (@bits $group:expr; $bits:tt: $uint:ty, $int:ty, $float:ty) => {
        bench_hash_functions!(@inputs $group; $bits: $uint, $int, $float; 1);
        bench_hash_functions!(@inputs $group; $bits: $uint, $int, $float; 2);
        bench_hash_functions!(@inputs $group; $bits: $uint, $int, $float; 3);
        bench_hash_functions!(@inputs $group; $bits: $uint, $int, $float; 4);
    };

    // Generate all type combinations for an input count
    (@inputs $group:expr; $bits:tt: $uint:ty, $int:ty, $float:ty; $n:tt) => {
        bench_hash_functions!(@types $group; $bits: $uint, $int, $float; $n; u u);
        bench_hash_functions!(@types $group; $bits: $uint, $int, $float; $n; u i);
        bench_hash_functions!(@types $group; $bits: $uint, $int, $float; $n; u f);
        bench_hash_functions!(@types $group; $bits: $uint, $int, $float; $n; i u);
        bench_hash_functions!(@types $group; $bits: $uint, $int, $float; $n; i i);
        bench_hash_functions!(@types $group; $bits: $uint, $int, $float; $n; i f);
    };

    // 1 input benchmarks
    (@types $group:expr; $bits:tt: $uint:ty, $int:ty, $float:ty; 1; u $out:tt) => {
        paste::paste! {
            $group.bench_function(
                BenchmarkId::new(stringify!([<hash11u $out $bits>]), ""),
                |b| b.iter(|| {
                    let mut sum = 0 as $uint;
                    for i in 0..ITERATIONS as $uint {
                        sum = sum.wrapping_add([<hash11u $out $bits>](black_box(i)) as $uint);
                    }
                    sum
                }),
            );
        }
    };
    (@types $group:expr; $bits:tt: $uint:ty, $int:ty, $float:ty; 1; i $out:tt) => {
        paste::paste! {
            $group.bench_function(
                BenchmarkId::new(stringify!([<hash11i $out $bits>]), ""),
                |b| b.iter(|| {
                    let mut sum = 0 as $int;
                    for i in 0..ITERATIONS as $int {
                        sum = sum.wrapping_add([<hash11i $out $bits>](black_box(i)) as $int);
                    }
                    sum
                }),
            );
        }
    };

    // 2 input benchmarks
    (@types $group:expr; $bits:tt: $uint:ty, $int:ty, $float:ty; 2; u $out:tt) => {
        paste::paste! {
            $group.bench_function(
                BenchmarkId::new(stringify!([<hash21u $out $bits>]), ""),
                |b| b.iter(|| {
                    let mut sum = 0 as $uint;
                    for i in 0..ITERATIONS as $uint {
                        sum = sum.wrapping_add([<hash21u $out $bits>](black_box(i), black_box(i ^ 0x5555)) as $uint);
                    }
                    sum
                }),
            );
        }
    };
    (@types $group:expr; $bits:tt: $uint:ty, $int:ty, $float:ty; 2; i $out:tt) => {
        paste::paste! {
            $group.bench_function(
                BenchmarkId::new(stringify!([<hash21i $out $bits>]), ""),
                |b| b.iter(|| {
                    let mut sum = 0 as $int;
                    for i in 0..ITERATIONS as $int {
                        sum = sum.wrapping_add([<hash21i $out $bits>](black_box(i), black_box(i ^ 0x5555)) as $int);
                    }
                    sum
                }),
            );
        }
    };

    // 3 input benchmarks
    (@types $group:expr; $bits:tt: $uint:ty, $int:ty, $float:ty; 3; u $out:tt) => {
        paste::paste! {
            $group.bench_function(
                BenchmarkId::new(stringify!([<hash31u $out $bits>]), ""),
                |b| b.iter(|| {
                    let mut sum = 0 as $uint;
                    for i in 0..ITERATIONS as $uint {
                        sum = sum.wrapping_add([<hash31u $out $bits>](black_box(i), black_box(i ^ 0x5555), black_box(i ^ 0xAAAA)) as $uint);
                    }
                    sum
                }),
            );
        }
    };
    (@types $group:expr; $bits:tt: $uint:ty, $int:ty, $float:ty; 3; i $out:tt) => {
        paste::paste! {
            $group.bench_function(
                BenchmarkId::new(stringify!([<hash31i $out $bits>]), ""),
                |b| b.iter(|| {
                    let mut sum = 0 as $int;
                    for i in 0..ITERATIONS as $int {
                        sum = sum.wrapping_add([<hash31i $out $bits>](black_box(i), black_box(i ^ 0x5555), black_box(i ^ 0xAAAA)) as $int);
                    }
                    sum
                }),
            );
        }
    };

    // 4 input benchmarks
    (@types $group:expr; $bits:tt: $uint:ty, $int:ty, $float:ty; 4; u $out:tt) => {
        paste::paste! {
            $group.bench_function(
                BenchmarkId::new(stringify!([<hash41u $out $bits>]), ""),
                |b| b.iter(|| {
                    let mut sum = 0 as $uint;
                    for i in 0..ITERATIONS as $uint {
                        sum = sum.wrapping_add([<hash41u $out $bits>](black_box(i), black_box(i ^ 0x5555), black_box(i ^ 0xAAAA), black_box(i ^ 0xFFFF)) as $uint);
                    }
                    sum
                }),
            );
        }
    };
    (@types $group:expr; $bits:tt: $uint:ty, $int:ty, $float:ty; 4; i $out:tt) => {
        paste::paste! {
            $group.bench_function(
                BenchmarkId::new(stringify!([<hash41i $out $bits>]), ""),
                |b| b.iter(|| {
                    let mut sum = 0 as $int;
                    for i in 0..ITERATIONS as $int {
                        sum = sum.wrapping_add([<hash41i $out $bits>](black_box(i), black_box(i ^ 0x5555), black_box(i ^ 0xAAAA), black_box(i ^ 0xFFFF)) as $int);
                    }
                    sum
                }),
            );
        }
    };
}

fn bench_hash_32(c: &mut Criterion) {
  let mut group = c.benchmark_group("hash/32bit");
  group.throughput(Throughput::Elements(ITERATIONS));

  bench_hash_functions!(group;
      32: u32, i32, f32
  );

  group.finish();
}

fn bench_hash_64(c: &mut Criterion) {
  let mut group = c.benchmark_group("hash/64bit");
  group.throughput(Throughput::Elements(ITERATIONS));

  bench_hash_functions!(group;
      64: u64, i64, f64
  );

  group.finish();
}

criterion_group!(benches, bench_hash_32, bench_hash_64);
criterion_main!(benches);
