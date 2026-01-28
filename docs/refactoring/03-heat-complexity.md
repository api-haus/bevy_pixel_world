# Heat Simulation Complexity Reduction

Reduce complexity in heat simulation functions.

## Target Functions

### 1. `ignite_from_heat()`

**Location:** `simulation/heat.rs:175-213`
**Cognitive Complexity:** 33
**Nesting Depth:** 6

### 2. `sample_heat_neighbors()`

**Location:** `simulation/heat.rs:73-124`
**Lines:** 52
**Issue:** Complex cross-chunk boundary logic

---

## `ignite_from_heat()` Refactoring

### Current Structure

```
ignite_from_heat()
├── for each chunk position
│   ├── get heat grid
│   └── for each heat cell
│       ├── calculate pixel region
│       └── for each pixel in region
│           ├── check if flammable
│           ├── get ignition threshold
│           └── if heat > threshold: ignite
```

### Proposed Decomposition

#### Extract: Process Single Heat Cell

```rust
/// Checks pixels in a heat cell region and ignites those exceeding threshold.
fn ignite_cell_region(
    pixels: &mut TilePixels,
    materials: &MaterialStore,
    heat_level: u8,
    cell_x: usize,
    cell_y: usize,
) {
    let base_x = cell_x * HEAT_CELL_SIZE;
    let base_y = cell_y * HEAT_CELL_SIZE;

    for dy in 0..HEAT_CELL_SIZE {
        for dx in 0..HEAT_CELL_SIZE {
            let px = base_x + dx;
            let py = base_y + dy;

            if let Some(pixel) = pixels.get_mut(px, py) {
                try_ignite_pixel(pixel, materials, heat_level);
            }
        }
    }
}

fn try_ignite_pixel(pixel: &mut Pixel, materials: &MaterialStore, heat: u8) {
    if pixel.flags.contains(PixelFlags::BURNING) {
        return;
    }
    let mat = materials.get(pixel.material);
    if let Some(threshold) = mat.ignition_threshold {
        if heat >= threshold {
            pixel.flags.insert(PixelFlags::BURNING | PixelFlags::DIRTY);
        }
    }
}
```

#### Refactored Main Function

```rust
pub fn ignite_from_heat(ctx: &mut SimContext) {
    for chunk_pos in ctx.world.chunk_positions() {
        let Some(heat_grid) = ctx.world.heat_grid(chunk_pos) else { continue };
        let Some(pixels) = ctx.world.pixels_mut(chunk_pos) else { continue };

        for (cell_idx, &heat_level) in heat_grid.iter().enumerate() {
            if heat_level == 0 { continue; }

            let cell_x = cell_idx % HEAT_GRID_SIZE;
            let cell_y = cell_idx / HEAT_GRID_SIZE;
            ignite_cell_region(pixels, &ctx.materials, heat_level, cell_x, cell_y);
        }
    }
}
```

---

## `sample_heat_neighbors()` Refactoring

### Current Issue

Complex coordinate arithmetic for cross-chunk boundary handling with `rem_euclid()` calls.

### Proposed: Extract Neighbor Coordinate Resolution

```rust
struct HeatCellCoord {
    chunk: ChunkPos,
    cell_x: usize,
    cell_y: usize,
}

impl HeatCellCoord {
    /// Resolve neighbor in given direction, handling chunk boundaries.
    fn neighbor(&self, dx: i32, dy: i32) -> HeatCellCoord {
        let new_x = self.cell_x as i32 + dx;
        let new_y = self.cell_y as i32 + dy;

        let (chunk_offset_x, cell_x) = wrap_cell_coord(new_x);
        let (chunk_offset_y, cell_y) = wrap_cell_coord(new_y);

        HeatCellCoord {
            chunk: self.chunk.offset(chunk_offset_x, chunk_offset_y),
            cell_x,
            cell_y,
        }
    }
}

fn wrap_cell_coord(coord: i32) -> (i32, usize) {
    if coord < 0 {
        (-1, (coord + HEAT_GRID_SIZE as i32) as usize)
    } else if coord >= HEAT_GRID_SIZE as i32 {
        (1, (coord - HEAT_GRID_SIZE as i32) as usize)
    } else {
        (0, coord as usize)
    }
}
```

### Refactored Sampling

```rust
pub fn sample_heat_neighbors(
    world: &World,
    chunk: ChunkPos,
    cell_x: usize,
    cell_y: usize,
) -> (u32, u32) {
    let coord = HeatCellCoord { chunk, cell_x, cell_y };
    let directions = [(−1, 0), (1, 0), (0, −1), (0, 1)];

    let mut sum = 0u32;
    let mut count = 0u32;

    for (dx, dy) in directions {
        let neighbor = coord.neighbor(dx, dy);
        if let Some(heat) = world.heat_at(neighbor.chunk, neighbor.cell_x, neighbor.cell_y) {
            sum += heat as u32;
            count += 1;
        }
    }

    (sum, count)
}
```

---

## Benefits

- Clearer separation of concerns
- Coordinate wrapping logic isolated and testable
- Main functions describe algorithm at high level

## Verification

```bash
cargo clippy -p bevy_pixel_world -- -D warnings
cargo build -p bevy_pixel_world
cargo test -p bevy_pixel_world

# Visual verification - fire spreading
cargo run --example fire
```

## Estimated Impact

- **Risk:** Low - behavior-preserving refactor
- **Lines changed:** ~40
- **Complexity reduction:** 33 → ~10 per function
