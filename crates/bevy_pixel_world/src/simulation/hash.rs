//! Deterministic hash functions for simulation randomness.

/// FNV-1a style mixing for 64-bit values.
#[inline]
fn mix64(mut h: u64) -> u64 {
  h = h.wrapping_mul(0x517c_c1b7_2722_0a95);
  h ^= h >> 32;
  h = h.wrapping_mul(0x517c_c1b7_2722_0a95);
  h ^= h >> 32;
  h
}

/// Hash 2 u64 inputs to 1 u64 output.
#[inline]
pub fn hash21uu64(a: u64, b: u64) -> u64 {
  mix64(a ^ b.rotate_left(32))
}

/// Hash 4 u64 inputs to 1 u64 output.
#[inline]
pub fn hash41uu64(a: u64, b: u64, c: u64, d: u64) -> u64 {
  mix64(a ^ b.rotate_left(16) ^ c.rotate_left(32) ^ d.rotate_left(48))
}
