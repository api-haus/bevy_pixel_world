# Blast Function Complexity Reduction

Decompose `blast()` to reduce cognitive complexity from 28 to ~10 per function.

## Overview

The blast function implements radial ray-casting for explosion effects with energy dissipation and heat injection. Currently 80 lines with complexity 28.

## Tasks
- [ ] Extract `cast_blast_rays()` for ray casting phase
- [ ] Extract `march_ray()` for individual ray marching
- [ ] Extract `awaken_blast_boundary()` for boundary awakening
- [ ] Extract `inject_blast_heat()` for heat injection
- [ ] Refactor main `blast()` to use extracted functions
- [ ] Add unit tests for extracted functions

## Refactoring Plan

### Data Structures
```rust
struct RaycastResult {
    affected_chunks: HashSet<ChunkPos>,
    blast_mask: BlastMask,
}
```

### Extracted Functions
```rust
fn cast_blast_rays<F>(
    world: &World,
    center: Vec2,
    radius: f32,
    energy: f32,
    on_hit: F,
) -> RaycastResult
where
    F: FnMut(PixelPos, &Pixel) -> f32;

fn march_ray<F>(
    world: &World,
    center: Vec2,
    dir: Vec2,
    max_dist: f32,
    initial_energy: f32,
    on_hit: &mut F,
    result: &mut RaycastResult,
);

fn awaken_blast_boundary(world: &mut World, affected_chunks: &HashSet<ChunkPos>);

fn inject_blast_heat(
    world: &mut World,
    center: Vec2,
    radius: f32,
    heat_intensity: u8,
);
```

### Refactored Main Function
```rust
pub fn blast<F>(
    world: &mut World,
    center: Vec2,
    radius: f32,
    energy: f32,
    heat_intensity: u8,
    on_hit: F,
) where
    F: FnMut(PixelPos, &Pixel) -> f32,
{
    let result = cast_blast_rays(world, center, radius, energy, on_hit);
    awaken_blast_boundary(world, &result.affected_chunks);
    inject_blast_heat(world, center, radius, heat_intensity);
}
```

## Benefits
- Clear three-phase structure: cast → awaken → heat
- Ray marching reusable for other effects (lasers, line-of-sight)
- Heat injection reusable for fire spread, lava, etc.

## Verification

```bash
cargo clippy -p bevy_pixel_world -- -D warnings
cargo build -p bevy_pixel_world
cargo test -p bevy_pixel_world

# Visual verification
cargo run -p game  # Test bomb explosions
```

## References
- docs/refactoring/04-blast-complexity.md
- Location: world/blast.rs:44-123
