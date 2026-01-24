//! Existence tests for hash functions.

use bevy_pixel_world::simulation::hash::*;

#[test]
fn all_hash_functions_exist() {
  // 32-bit: 1 input
  let _ = hash11uu32(0);
  let _ = hash11ui32(0);
  let _ = hash11uf32(0);
  let _ = hash11iu32(0);
  let _ = hash11ii32(0);
  let _ = hash11if32(0);

  // 32-bit: 2 inputs
  let _ = hash21uu32(0, 0);
  let _ = hash21ui32(0, 0);
  let _ = hash21uf32(0, 0);
  let _ = hash21iu32(0, 0);
  let _ = hash21ii32(0, 0);
  let _ = hash21if32(0, 0);

  // 32-bit: 3 inputs
  let _ = hash31uu32(0, 0, 0);
  let _ = hash31ui32(0, 0, 0);
  let _ = hash31uf32(0, 0, 0);
  let _ = hash31iu32(0, 0, 0);
  let _ = hash31ii32(0, 0, 0);
  let _ = hash31if32(0, 0, 0);

  // 32-bit: 4 inputs
  let _ = hash41uu32(0, 0, 0, 0);
  let _ = hash41ui32(0, 0, 0, 0);
  let _ = hash41uf32(0, 0, 0, 0);
  let _ = hash41iu32(0, 0, 0, 0);
  let _ = hash41ii32(0, 0, 0, 0);
  let _ = hash41if32(0, 0, 0, 0);

  // 64-bit: 1 input
  let _ = hash11uu64(0);
  let _ = hash11ui64(0);
  let _ = hash11uf64(0);
  let _ = hash11iu64(0);
  let _ = hash11ii64(0);
  let _ = hash11if64(0);

  // 64-bit: 2 inputs
  let _ = hash21uu64(0, 0);
  let _ = hash21ui64(0, 0);
  let _ = hash21uf64(0, 0);
  let _ = hash21iu64(0, 0);
  let _ = hash21ii64(0, 0);
  let _ = hash21if64(0, 0);

  // 64-bit: 3 inputs
  let _ = hash31uu64(0, 0, 0);
  let _ = hash31ui64(0, 0, 0);
  let _ = hash31uf64(0, 0, 0);
  let _ = hash31iu64(0, 0, 0);
  let _ = hash31ii64(0, 0, 0);
  let _ = hash31if64(0, 0, 0);

  // 64-bit: 4 inputs
  let _ = hash41uu64(0, 0, 0, 0);
  let _ = hash41ui64(0, 0, 0, 0);
  let _ = hash41uf64(0, 0, 0, 0);
  let _ = hash41iu64(0, 0, 0, 0);
  let _ = hash41ii64(0, 0, 0, 0);
  let _ = hash41if64(0, 0, 0, 0);
}
