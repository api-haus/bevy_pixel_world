# Blast Function Complexity Reduction

Decompose `blast()` to reduce cognitive complexity.

## Current State

**Location:** `world/blast.rs:44-123`
**Cognitive Complexity:** 28
**Lines:** 80

The function implements radial ray-casting for explosion effects with energy dissipation and heat injection.

## Current Structure

```
blast()
├── Calculate ray count from circumference
├── For each ray angle
│   ├── Calculate direction vector
│   ├── Ray march loop
│   │   ├── Step along ray
│   │   ├── Check pixel at position
│   │   ├── Invoke callback for energy cost
│   │   └── Stop if energy depleted
│   └── Track affected chunks
├── Awaken boundary pixels
└── Inject heat with spherical falloff
```

## Proposed Decomposition

### Extract 1: Ray Casting Phase

```rust
struct RaycastResult {
    affected_chunks: HashSet<ChunkPos>,
    blast_mask: BlastMask,  // or whatever tracks destroyed pixels
}

/// Cast rays outward from center, invoking callback for each hit.
fn cast_blast_rays<F>(
    world: &World,
    center: Vec2,
    radius: f32,
    energy: f32,
    mut on_hit: F,
) -> RaycastResult
where
    F: FnMut(PixelPos, &Pixel) -> f32,  // returns energy cost
{
    let ray_count = (2.0 * PI * radius).ceil() as usize;
    let mut result = RaycastResult::default();

    for i in 0..ray_count {
        let angle = (i as f32 / ray_count as f32) * 2.0 * PI;
        let dir = Vec2::new(angle.cos(), angle.sin());

        march_ray(world, center, dir, radius, energy, &mut on_hit, &mut result);
    }

    result
}

fn march_ray<F>(
    world: &World,
    center: Vec2,
    dir: Vec2,
    max_dist: f32,
    initial_energy: f32,
    on_hit: &mut F,
    result: &mut RaycastResult,
) where
    F: FnMut(PixelPos, &Pixel) -> f32,
{
    let mut energy = initial_energy;
    let mut dist = 0.0;

    while dist < max_dist && energy > 0.0 {
        let pos = center + dir * dist;
        let pixel_pos = pos.as_pixel_pos();

        if let Some(pixel) = world.get_pixel(pixel_pos) {
            if !pixel.is_void() {
                let cost = on_hit(pixel_pos, pixel);
                energy -= cost;
                result.affected_chunks.insert(pixel_pos.chunk());
            }
        }

        dist += 1.0;
    }
}
```

### Extract 2: Boundary Awakening

```rust
/// Wake pixels at explosion boundary so exposed material falls/flows.
fn awaken_blast_boundary(world: &mut World, affected_chunks: &HashSet<ChunkPos>) {
    for &chunk in affected_chunks {
        world.wake_chunk_boundary(chunk);
    }
}
```

### Extract 3: Heat Injection

```rust
/// Inject heat with smooth spherical falloff.
fn inject_blast_heat(
    world: &mut World,
    center: Vec2,
    radius: f32,
    heat_intensity: u8,
) {
    let radius_sq = radius * radius;

    // Iterate heat cells within bounding box
    for cell in heat_cells_in_radius(center, radius) {
        let cell_center = cell.center_position();
        let dist_sq = center.distance_squared(cell_center);

        if dist_sq < radius_sq {
            // Smooth falloff: 1.0 at center, 0.0 at edge
            let falloff = 1.0 - (dist_sq / radius_sq).sqrt();
            let heat = (heat_intensity as f32 * falloff) as u8;
            world.add_heat(cell, heat);
        }
    }
}
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
- Each phase independently testable
- Ray marching logic reusable for other effects (lasers, line-of-sight)
- Heat injection could be reused for fire spread, lava, etc.

## Verification

```bash
cargo clippy -p bevy_pixel_world -- -D warnings
cargo build -p bevy_pixel_world
cargo test -p bevy_pixel_world

# Visual verification
cargo run --example bombs
```

## Estimated Impact

- **Risk:** Low - behavior-preserving refactor
- **Lines changed:** ~30 (mostly reorganization)
- **Complexity reduction:** 28 → ~10 per function
- **Reusability:** Ray march and heat injection become reusable primitives
