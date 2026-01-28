# Bomb Shell Complexity Reduction

Decompose `compute_bomb_shell()` to reduce cognitive complexity.

## Current State

**Location:** `pixel_body/bomb.rs:38-115`
**Cognitive Complexity:** 48
**Lines:** 78

The function performs BFS to find shell pixels (solid pixels near the surface) for bomb detonation effects.

## Current Structure

```
compute_bomb_shell()
├── Calculate shell depth threshold
├── Initialize distance tracking
├── Seed BFS queue with edge-adjacent pixels
│   ├── Check all boundary pixels
│   └── Check interior pixels adjacent to void
├── BFS propagation loop
│   ├── Pop from queue
│   ├── Check 4 neighbors
│   └── Add valid neighbors to queue
└── Build final shell mask
```

## Proposed Decomposition

### Extract 1: Edge Detection Seeding

```rust
/// Seeds BFS queue with solid pixels adjacent to void or image boundary.
fn seed_boundary_pixels(
    body: &PixelBody,
    distances: &mut [i32],
    queue: &mut VecDeque<(i32, i32)>,
) {
    // Current lines 52-80
}
```

### Extract 2: BFS Propagation

```rust
/// Propagates distance values inward from seeded boundary pixels.
fn propagate_distances(
    body: &PixelBody,
    distances: &mut [i32],
    queue: &mut VecDeque<(i32, i32)>,
    max_depth: i32,
) {
    // Current lines 82-102
}
```

### Extract 3: Shell Mask Creation

```rust
/// Creates shell mask from computed distances.
fn build_shell_mask(
    distances: &[i32],
    width: u32,
    height: u32,
    depth: i32,
) -> BombShellMask {
    // Current lines 104-114
}
```

### Refactored Main Function

```rust
pub fn compute_bomb_shell(body: &PixelBody) -> BombShellMask {
    let (width, height) = (body.width, body.height);
    let depth = calculate_shell_depth(width, height);

    let mut distances = vec![-1i32; (width * height) as usize];
    let mut queue = VecDeque::new();

    seed_boundary_pixels(body, &mut distances, &mut queue);
    propagate_distances(body, &mut distances, &mut queue, depth);
    build_shell_mask(&distances, width, height, depth)
}

fn calculate_shell_depth(width: u32, height: u32) -> i32 {
    let half_dim = width.min(height) as i32 / 2;
    (half_dim / 10).max(1)
}
```

## Benefits

- Each helper has single responsibility
- Main function reads as high-level algorithm description
- Helpers are independently testable
- Reduces cognitive load when reading

## Verification

```bash
cargo clippy -p bevy_pixel_world -- -D warnings
cargo build -p bevy_pixel_world
cargo test -p bevy_pixel_world

# Visual verification
cargo run --example bombs
```

## Estimated Impact

- **Risk:** Low - pure refactoring, no behavior change
- **Lines changed:** ~20 (added function signatures, moved code)
- **Complexity reduction:** 48 → ~12 per function
