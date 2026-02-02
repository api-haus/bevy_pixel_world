# Bomb Shell Complexity Reduction

Decompose `compute_bomb_shell()` to reduce cognitive complexity from 48 to ~12 per function.

## Overview

The function performs BFS to find shell pixels (solid pixels near the surface) for bomb detonation effects. Currently 78 lines with complexity 48.

## Tasks
- [ ] Extract `seed_boundary_pixels()` for BFS queue seeding
- [ ] Extract `propagate_distances()` for BFS propagation loop
- [ ] Extract `build_shell_mask()` for final mask creation
- [ ] Extract `calculate_shell_depth()` helper
- [ ] Refactor main `compute_bomb_shell()` to use extracted functions
- [ ] Run tests to verify behavior unchanged

## Refactoring Plan

### Current Structure
```
compute_bomb_shell()
├── Calculate shell depth threshold (lines 46-49)
├── Initialize distance tracking (lines 51)
├── Seed BFS queue (lines 52-80)
├── BFS propagation loop (lines 82-102)
└── Build final shell mask (lines 104-114)
```

### Extracted Functions
```rust
fn seed_boundary_pixels(
    body: &PixelBody,
    distances: &mut [i32],
    queue: &mut VecDeque<(i32, i32)>,
);

fn propagate_distances(
    body: &PixelBody,
    distances: &mut [i32],
    queue: &mut VecDeque<(i32, i32)>,
    max_depth: i32,
);

fn build_shell_mask(
    distances: &[i32],
    width: u32,
    height: u32,
    depth: i32,
) -> BombShellMask;

fn calculate_shell_depth(width: u32, height: u32) -> i32;
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
```

## Verification

```bash
cargo clippy -p bevy_pixel_world -- -D warnings
cargo build -p bevy_pixel_world
cargo test -p bevy_pixel_world

# Visual verification
cargo run -p game  # Test bomb explosions
```

## References
- docs/refactoring/02-bomb-complexity.md
- Location: pixel_body/bomb.rs:38-115
