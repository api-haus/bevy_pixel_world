# Heat Simulation Complexity Reduction

Reduce complexity in heat simulation functions.

## Overview

Target functions in simulation/heat.rs have excessive cognitive complexity and need decomposition.

## Tasks

### `ignite_from_heat()` (lines 175-213, complexity 33)
- [ ] Extract `ignite_cell_region()` for processing single heat cell
- [ ] Extract `try_ignite_pixel()` for individual pixel ignition check
- [ ] Refactor main function to use extracted helpers
- [ ] Reduce nesting depth from 6 to 2-3

### `sample_heat_neighbors()` (lines 73-124, 52 lines)
- [ ] Create `HeatCellCoord` struct for coordinate handling
- [ ] Extract `neighbor()` method for boundary wrapping
- [ ] Extract `wrap_cell_coord()` helper
- [ ] Refactor sampling to use coordinate abstraction

## Refactoring Plan

### Ignite from Heat
```rust
fn ignite_cell_region(
    pixels: &mut TilePixels,
    materials: &MaterialStore,
    heat_level: u8,
    cell_x: usize,
    cell_y: usize,
);

fn try_ignite_pixel(pixel: &mut Pixel, materials: &MaterialStore, heat: u8);
```

### Heat Cell Coordinates
```rust
struct HeatCellCoord {
    chunk: ChunkPos,
    cell_x: usize,
    cell_y: usize,
}

impl HeatCellCoord {
    fn neighbor(&self, dx: i32, dy: i32) -> HeatCellCoord;
}

fn wrap_cell_coord(coord: i32) -> (i32, usize);
```

## Verification

```bash
cargo clippy -p game -- -D warnings
cargo build -p game
cargo test -p game

# Visual verification - fire spreading
cargo run -p game  # Test fire/burning materials
```

## References
- docs/refactoring/03-heat-complexity.md
