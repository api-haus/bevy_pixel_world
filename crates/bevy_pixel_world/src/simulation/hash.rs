//! Deterministic hash functions for simulation randomness.
//!
//! Provides reproducible pseudo-random values based on input parameters.
//!
//! # Naming Convention
//!
//! `hash{inputs}{outputs}{input_type}{output_type}{bits}`
//!
//! - **inputs**: Number of input parameters (1-4)
//! - **outputs**: Number of outputs (always 1)
//! - **input_type**: `u` = unsigned, `i` = signed
//! - **output_type**: `u` = unsigned, `i` = signed, `f` = float (0..1 range)
//! - **bits**: `32` or `64`
//!
//! # Examples
//!
//! - `hash11uu32` - 1 u32 in, 1 u32 out
//! - `hash41uu64` - 4 u64 in, 1 u64 out
//! - `hash21uf32` - 2 u32 in, 1 f32 out (0..1)
//! - `hash31if64` - 3 i64 in, 1 f64 out (0..1)

/// FNV-1a style mixing for 32-bit values.
#[inline]
fn mix32(mut h: u32) -> u32 {
  h = h.wrapping_mul(0x517c_c1b7);
  h ^= h >> 16;
  h = h.wrapping_mul(0x517c_c1b7);
  h ^= h >> 16;
  h
}

/// FNV-1a style mixing for 64-bit values.
#[inline]
fn mix64(mut h: u64) -> u64 {
  h = h.wrapping_mul(0x517c_c1b7_2722_0a95);
  h ^= h >> 32;
  h = h.wrapping_mul(0x517c_c1b7_2722_0a95);
  h ^= h >> 32;
  h
}

/// Convert u32 to f32 in [0.0, 1.0) range.
#[inline]
fn to_frac32(h: u32) -> f32 {
  (h >> 9) as f32 * (1.0 / (1u32 << 23) as f32)
}

/// Convert u64 to f64 in [0.0, 1.0) range.
#[inline]
fn to_frac64(h: u64) -> f64 {
  (h >> 12) as f64 * (1.0 / (1u64 << 52) as f64)
}

/// Cartesian product macro - calls callback with each combination.
macro_rules! cartesian {
  // Entry point
  ($callback:ident ($($fixed:tt)*); $($lists:tt)+) => {
    cartesian!(@step $callback ($($fixed)*) () $($lists)+);
  };
  // Recurse: pick each item from first list, continue with rest
  (@step $callback:ident ($($fixed:tt)*) ($($acc:tt)*) [$first:tt $($more:tt)*] $($rest:tt)*) => {
    cartesian!(@step $callback ($($fixed)*) ($($acc)* $first) $($rest)*);
    cartesian!(@step $callback ($($fixed)*) ($($acc)*) [$($more)*] $($rest)*);
  };
  // Skip empty list
  (@step $callback:ident ($($fixed:tt)*) ($($acc:tt)*) [] $($rest:tt)*) => {};
  // Base case: no more lists, emit
  (@step $callback:ident ($($fixed:tt)*) ($($acc:tt)*)) => {
    $callback!($($fixed)* $($acc)*);
  };
}

/// Generate a single hash function.
macro_rules! make_hash {
  // 1 input
  ($bits:tt $uint:ty, $int:ty, $float:ty, $mix:ident, $frac:ident; 1 u u) => {
    paste::paste! {
      #[allow(dead_code)]
      #[inline]
      pub fn [<hash11uu $bits>](a: $uint) -> $uint { $mix(a) }
    }
  };
  ($bits:tt $uint:ty, $int:ty, $float:ty, $mix:ident, $frac:ident; 1 u i) => {
    paste::paste! {
      #[allow(dead_code)]
      #[inline]
      pub fn [<hash11ui $bits>](a: $uint) -> $int { $mix(a) as $int }
    }
  };
  ($bits:tt $uint:ty, $int:ty, $float:ty, $mix:ident, $frac:ident; 1 u f) => {
    paste::paste! {
      #[allow(dead_code)]
      #[inline]
      pub fn [<hash11uf $bits>](a: $uint) -> $float { $frac($mix(a)) }
    }
  };
  ($bits:tt $uint:ty, $int:ty, $float:ty, $mix:ident, $frac:ident; 1 i u) => {
    paste::paste! {
      #[allow(dead_code)]
      #[inline]
      pub fn [<hash11iu $bits>](a: $int) -> $uint { $mix(a as $uint) }
    }
  };
  ($bits:tt $uint:ty, $int:ty, $float:ty, $mix:ident, $frac:ident; 1 i i) => {
    paste::paste! {
      #[allow(dead_code)]
      #[inline]
      pub fn [<hash11ii $bits>](a: $int) -> $int { $mix(a as $uint) as $int }
    }
  };
  ($bits:tt $uint:ty, $int:ty, $float:ty, $mix:ident, $frac:ident; 1 i f) => {
    paste::paste! {
      #[allow(dead_code)]
      #[inline]
      pub fn [<hash11if $bits>](a: $int) -> $float { $frac($mix(a as $uint)) }
    }
  };

  // 2 inputs
  ($bits:tt $uint:ty, $int:ty, $float:ty, $mix:ident, $frac:ident; 2 u u) => {
    paste::paste! {
      #[allow(dead_code)]
      #[inline]
      pub fn [<hash21uu $bits>](a: $uint, b: $uint) -> $uint {
        $mix(a ^ b.rotate_left($bits / 2))
      }
    }
  };
  ($bits:tt $uint:ty, $int:ty, $float:ty, $mix:ident, $frac:ident; 2 u i) => {
    paste::paste! {
      #[allow(dead_code)]
      #[inline]
      pub fn [<hash21ui $bits>](a: $uint, b: $uint) -> $int {
        $mix(a ^ b.rotate_left($bits / 2)) as $int
      }
    }
  };
  ($bits:tt $uint:ty, $int:ty, $float:ty, $mix:ident, $frac:ident; 2 u f) => {
    paste::paste! {
      #[allow(dead_code)]
      #[inline]
      pub fn [<hash21uf $bits>](a: $uint, b: $uint) -> $float {
        $frac($mix(a ^ b.rotate_left($bits / 2)))
      }
    }
  };
  ($bits:tt $uint:ty, $int:ty, $float:ty, $mix:ident, $frac:ident; 2 i u) => {
    paste::paste! {
      #[allow(dead_code)]
      #[inline]
      pub fn [<hash21iu $bits>](a: $int, b: $int) -> $uint {
        $mix((a as $uint) ^ (b as $uint).rotate_left($bits / 2))
      }
    }
  };
  ($bits:tt $uint:ty, $int:ty, $float:ty, $mix:ident, $frac:ident; 2 i i) => {
    paste::paste! {
      #[allow(dead_code)]
      #[inline]
      pub fn [<hash21ii $bits>](a: $int, b: $int) -> $int {
        $mix((a as $uint) ^ (b as $uint).rotate_left($bits / 2)) as $int
      }
    }
  };
  ($bits:tt $uint:ty, $int:ty, $float:ty, $mix:ident, $frac:ident; 2 i f) => {
    paste::paste! {
      #[allow(dead_code)]
      #[inline]
      pub fn [<hash21if $bits>](a: $int, b: $int) -> $float {
        $frac($mix((a as $uint) ^ (b as $uint).rotate_left($bits / 2)))
      }
    }
  };

  // 3 inputs
  ($bits:tt $uint:ty, $int:ty, $float:ty, $mix:ident, $frac:ident; 3 u u) => {
    paste::paste! {
      #[allow(dead_code)]
      #[inline]
      pub fn [<hash31uu $bits>](a: $uint, b: $uint, c: $uint) -> $uint {
        $mix(a ^ b.rotate_left($bits / 3) ^ c.rotate_left(2 * $bits / 3))
      }
    }
  };
  ($bits:tt $uint:ty, $int:ty, $float:ty, $mix:ident, $frac:ident; 3 u i) => {
    paste::paste! {
      #[allow(dead_code)]
      #[inline]
      pub fn [<hash31ui $bits>](a: $uint, b: $uint, c: $uint) -> $int {
        $mix(a ^ b.rotate_left($bits / 3) ^ c.rotate_left(2 * $bits / 3)) as $int
      }
    }
  };
  ($bits:tt $uint:ty, $int:ty, $float:ty, $mix:ident, $frac:ident; 3 u f) => {
    paste::paste! {
      #[allow(dead_code)]
      #[inline]
      pub fn [<hash31uf $bits>](a: $uint, b: $uint, c: $uint) -> $float {
        $frac($mix(a ^ b.rotate_left($bits / 3) ^ c.rotate_left(2 * $bits / 3)))
      }
    }
  };
  ($bits:tt $uint:ty, $int:ty, $float:ty, $mix:ident, $frac:ident; 3 i u) => {
    paste::paste! {
      #[allow(dead_code)]
      #[inline]
      pub fn [<hash31iu $bits>](a: $int, b: $int, c: $int) -> $uint {
        $mix((a as $uint) ^ (b as $uint).rotate_left($bits / 3) ^ (c as $uint).rotate_left(2 * $bits / 3))
      }
    }
  };
  ($bits:tt $uint:ty, $int:ty, $float:ty, $mix:ident, $frac:ident; 3 i i) => {
    paste::paste! {
      #[allow(dead_code)]
      #[inline]
      pub fn [<hash31ii $bits>](a: $int, b: $int, c: $int) -> $int {
        $mix((a as $uint) ^ (b as $uint).rotate_left($bits / 3) ^ (c as $uint).rotate_left(2 * $bits / 3)) as $int
      }
    }
  };
  ($bits:tt $uint:ty, $int:ty, $float:ty, $mix:ident, $frac:ident; 3 i f) => {
    paste::paste! {
      #[allow(dead_code)]
      #[inline]
      pub fn [<hash31if $bits>](a: $int, b: $int, c: $int) -> $float {
        $frac($mix((a as $uint) ^ (b as $uint).rotate_left($bits / 3) ^ (c as $uint).rotate_left(2 * $bits / 3)))
      }
    }
  };

  // 4 inputs
  ($bits:tt $uint:ty, $int:ty, $float:ty, $mix:ident, $frac:ident; 4 u u) => {
    paste::paste! {
      #[allow(dead_code)]
      #[inline]
      pub fn [<hash41uu $bits>](a: $uint, b: $uint, c: $uint, d: $uint) -> $uint {
        $mix(a ^ b.rotate_left($bits / 4) ^ c.rotate_left($bits / 2) ^ d.rotate_left(3 * $bits / 4))
      }
    }
  };
  ($bits:tt $uint:ty, $int:ty, $float:ty, $mix:ident, $frac:ident; 4 u i) => {
    paste::paste! {
      #[allow(dead_code)]
      #[inline]
      pub fn [<hash41ui $bits>](a: $uint, b: $uint, c: $uint, d: $uint) -> $int {
        $mix(a ^ b.rotate_left($bits / 4) ^ c.rotate_left($bits / 2) ^ d.rotate_left(3 * $bits / 4)) as $int
      }
    }
  };
  ($bits:tt $uint:ty, $int:ty, $float:ty, $mix:ident, $frac:ident; 4 u f) => {
    paste::paste! {
      #[allow(dead_code)]
      #[inline]
      pub fn [<hash41uf $bits>](a: $uint, b: $uint, c: $uint, d: $uint) -> $float {
        $frac($mix(a ^ b.rotate_left($bits / 4) ^ c.rotate_left($bits / 2) ^ d.rotate_left(3 * $bits / 4)))
      }
    }
  };
  ($bits:tt $uint:ty, $int:ty, $float:ty, $mix:ident, $frac:ident; 4 i u) => {
    paste::paste! {
      #[allow(dead_code)]
      #[inline]
      pub fn [<hash41iu $bits>](a: $int, b: $int, c: $int, d: $int) -> $uint {
        $mix((a as $uint) ^ (b as $uint).rotate_left($bits / 4) ^ (c as $uint).rotate_left($bits / 2) ^ (d as $uint).rotate_left(3 * $bits / 4))
      }
    }
  };
  ($bits:tt $uint:ty, $int:ty, $float:ty, $mix:ident, $frac:ident; 4 i i) => {
    paste::paste! {
      #[allow(dead_code)]
      #[inline]
      pub fn [<hash41ii $bits>](a: $int, b: $int, c: $int, d: $int) -> $int {
        $mix((a as $uint) ^ (b as $uint).rotate_left($bits / 4) ^ (c as $uint).rotate_left($bits / 2) ^ (d as $uint).rotate_left(3 * $bits / 4)) as $int
      }
    }
  };
  ($bits:tt $uint:ty, $int:ty, $float:ty, $mix:ident, $frac:ident; 4 i f) => {
    paste::paste! {
      #[allow(dead_code)]
      #[inline]
      pub fn [<hash41if $bits>](a: $int, b: $int, c: $int, d: $int) -> $float {
        $frac($mix((a as $uint) ^ (b as $uint).rotate_left($bits / 4) ^ (c as $uint).rotate_left($bits / 2) ^ (d as $uint).rotate_left(3 * $bits / 4)))
      }
    }
  };
}

/// Generate all hash functions for a given bit width.
macro_rules! define_hashes {
  ($bits:tt, $uint:ty, $int:ty, $float:ty, $mix:ident, $frac:ident) => {
    cartesian!(make_hash ($bits $uint, $int, $float, $mix, $frac;); [1 2 3 4] [u i] [u i f]);
  };
}

define_hashes!(32, u32, i32, f32, mix32, to_frac32);
define_hashes!(64, u64, i64, f64, mix64, to_frac64);
