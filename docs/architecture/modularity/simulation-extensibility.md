# Simulation Extensibility

> **Status: Planned Architecture**
>
> This document describes simulation patterns for games. All simulations are game-implemented. The framework provides iteration primitives; games implement rules.

Pluggable simulation rules and reusable library functions.

## Philosophy

**Framework provides:**
- Iteration primitives (checkerboard phasing, sequential, parallel)
- Dirty tracking infrastructure
- Bevy system scheduling integration

**Game implements:**
- All simulation rules (falling sand, burning, melting)
- Material interactions
- Heat diffusion
- Any game-specific behavior

## Core Model

A simulation is a Bevy system combined with a schedule mode:

```
Simulation = Bevy System + ScheduleMode
```

**Bevy System:**
- Function parameters declare layer access (`Res`/`ResMut`)
- Writes directly to layer data
- Layer dependencies inferred → Bevy handles inter-system parallelism

**Schedule Mode** (pixel iteration within the system):
- `PhasedParallel`: Checkerboard 4-phase (spatial isolation, safe for local ops)
- `Sequential`: One pixel at a time (always safe, for global state)

**Swap-follow:** When `Pixel` swaps, all swap-layers in the bundle swap atomically. Single memory operation, no per-layer loop.

## Two Levels of Parallelism

| Level | Mechanism | Safety |
|-------|-----------|--------|
| Between systems | Bevy scheduler (disjoint layer writes → parallel) | Automatic |
| Within system | Schedule mode (how pixels iterated) | Mode-dependent |

**Example:**

```
FallingSandSim: writes Material, Flags  → PhasedParallel
HeatDiffusionSim: writes Heat           → PhasedParallel
→ Run in parallel (disjoint write sets)

DecaySim: writes Material, Damage       → Sequential
InteractionSim: writes Material, Damage → Sequential
→ Run sequentially (shared writes, Bevy orders them)
```

## Schedule Modes

The iterator type determines the schedule mode:

| Iterator | Mechanism | Safety | Use Case |
|----------|-----------|--------|----------|
| `PhasedIter<L>` | Checkerboard 4-phase | Safe for local ops | Standard CA physics |
| (regular loop) | Sequential iteration | Always safe | Global state, complex deps |

### PhasedIter

System runs 4 times per tick. Each run, the iterator yields only tiles of one phase. Barrier between phases ensures spatial isolation.

See [Scheduling](../simulation/scheduling.md) for checkerboard mechanics.

### Sequential

No special iterator - just use `canvas.iter_all()` or similar. Single-threaded, can safely mutate global state.

## Defining Simulations

Simulations are plain Bevy systems. The **iterator type** determines the schedule mode, and **layers** provide global pixel addressing with swap operations.

### Iterator API (Closure-based)

Iterators use a closure pattern (not Rust's Iterator trait) for internal parallelism:

```rust
/// Checkerboard 4-phase iteration (safe for local ops)
struct PhasedIter<'w, L: Layer> { ... }

impl<L: Layer> PhasedIter<'_, L> {
    /// Iterate all positions, internally parallelized by phase
    fn for_each<F>(&self, f: F)
    where
        F: Fn(WorldFragment) + Send + Sync;
}

// Sequential: just use layer.iter_all(), no special iterator needed
```

### WorldFragment

The closure receives a `WorldFragment` with position and context:

```rust
struct WorldFragment {
    pos: WorldPos,
    // Normalized coordinates, etc.
}

impl WorldFragment {
    fn pos(&self) -> WorldPos;
}
```

### SimContext (Separate Resource)

Simulation context (tick, seed, jitter) is a separate resource, not part of WorldFragment:

```rust
#[derive(Resource)]
struct SimContext {
    tick: u64,
    seed: u64,
    jitter: u8,
}
```

### Layers and PixelAccess

Separate layers define their data type and sample rate. The framework provides generic access traits:

```rust
// User defines layer properties
trait Layer {
    type Element: Copy + Default;
    const SAMPLE_RATE: u32;  // 1 = per-pixel, 4 = 4×4 regions, etc.
    const NAME: &'static str;
}

// Framework provides automatically for all layers
trait PixelAccess {
    fn get(&self, pos: WorldPos) -> Self::Element;
    fn set(&mut self, pos: WorldPos, value: Self::Element);
    fn swap(&mut self, a: WorldPos, b: WorldPos);
    fn swap_unchecked(&mut self, a: WorldPos, b: WorldPos);
    fn iter_all(&self) -> impl Iterator<Item = WorldPos>;
}
```

**Swap-follow:** When `Pixel` swaps, all swap-layers in the bundle swap atomically. Single memory operation.

For downsampled layers, divide coordinates explicitly:

```rust
// Heat layer has sample_rate: 4
let heat = heat_layer.get(local_pos / 4);
```

No implicit conversions—addressing stays transparent.

### PhasedIter (safe parallel)

```rust
fn falling_sand_sim(
    iter: PhasedIter<Pixel>,
    mut pixels: LayerMut<Pixel>,
    materials: Res<MaterialRegistry>,
) {
    iter.for_each(|frag| {
        let pos = frag.pos();
        let pixel = pixels.get(pos);
        if pixel.is_void() { return; }

        let below = pos.below();  // may be in neighbor chunk — handled uniformly
        if can_displace(&pixel, &pixels.get(below), &materials) {
            pixels.swap(pos, below);
        }
    });
}
```

`PhasedIter<L>::for_each()` internally:
1. Iterates phase A tiles via `rayon::par_iter()`
2. Dirty rect tracking skips dormant tiles (~90% under typical load)
3. Barrier
4. Phase B, C, D...

### Sequential (no special iterator)

```rust
fn complex_interaction_sim(
    mut pixels: LayerMut<Pixel>,
    mut global_state: ResMut<InteractionState>,
) {
    for pos in pixels.iter_all() {
        // Regular iteration, single-threaded
        // Can safely mutate global state
        global_state.process(pos, &mut pixels);
    }
}
```

Just a normal Bevy system with regular iteration. Use when you need global state or complex dependencies.

## System Ordering API

Mimics Bevy's tuple and `.chain()` semantics:

- **Tuples** = run in parallel (if layer deps allow)
- **`.chain()`** = force sequential order

```rust
// Game crate - all bundles, layers, and simulations are game-defined
PixelWorldPlugin::builder()
    .with_bundle(FallingSandBundle)  // Game's bundle (Pixel + Color + Damage)
    .with_positional::<HeatLayer>()  // Game's positional layer

    // Parallel group: tuple without .chain()
    .with_simulations((
        falling_sand_sim,   // PhasedIter<Pixel>, writes Pixel
        heat_diffusion_sim, // PhasedIter<HeatLayer>, writes HeatLayer
    ))

    // Sequential group: .chain() forces order
    .with_simulations((
        decay_sim,        // sequential, writes Pixel
        interaction_sim,  // sequential, writes Pixel
    ).chain())

    .build()
```

**Execution flow:**

```
┌─────────────────────────────────────────────────────┐
│  falling_sand_sim ──┬──► run in parallel            │
│  heat_diffusion_sim ┘   (disjoint layer writes)     │
└─────────────────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────┐
│  decay_sim ────────► interaction_sim                │
│                      (chained, same layer)          │
└─────────────────────────────────────────────────────┘
```

### Example: Minimal Physics Game

```rust
// Game with just Pixel layer (no color, damage, etc.)
PixelWorldPlugin::builder()
    .with_bundle(MinimalGameBundle)  // Game's minimal bundle (Pixel only)
    .with_simulations((
        simple_falling_sim,  // PhasedIter<Pixel>
    ))
    .build()
```

### Example: Thermal Sandbox

```rust
// Game with thermal layers
PixelWorldPlugin::builder()
    .with_bundle(FallingSandBundle)       // Game's bundle
    .with_positional::<HeatLayer>()       // Game's positional layers
    .with_positional::<PressureLayer>()

    // Physics + diffusion in parallel (disjoint layer writes)
    .with_simulations((
        falling_sand_sim,    // PhasedIter<Pixel>
        heat_diffusion_sim,  // PhasedIter<HeatLayer>
        pressure_sim,        // PhasedIter<PressureLayer>
    ))

    // Reactions chained after (same layer)
    .with_simulations((
        thermal_melting_sim, // sequential, writes Pixel
        explosion_sim,       // sequential, writes Pixel
    ).chain())

    .build()
```

## Library vs Demo Functions

The framework provides **library functions**—generic building blocks that operate on `Pixel` values. The **demo game** provides example physics implementations that users copy and customize.

### Library Functions (Engine Crate)

Generic helpers that work with any material configuration:

```rust
/// Parametric dispersion for gases and liquids.
/// Returns offset direction based on dispersion factor.
pub fn disperse(
    dispersion: u8,
    tick: u64,
    pos: WorldPos,
) -> (i64, i64)  // (dx, dy) offset

/// Count neighbors matching a predicate.
pub fn neighbors_matching<F>(
    pos: WorldPos,
    get_pixel: F,
) -> u8
where
    F: Fn(WorldPos) -> Option<Pixel>

/// Check if source can displace target based on density.
pub fn can_displace(
    src: &Pixel,
    dst: &Pixel,
    materials: &MaterialRegistry,
) -> bool

/// Pixel-level raycasting.
pub fn raycast(
    origin: WorldPos,
    direction: (f32, f32),
    get_pixel: impl Fn(WorldPos) -> Option<Pixel>,
    max_dist: f32,
) -> RaycastHit
```

### Demo Functions (Demo Game)

Example implementations that users copy into their games and modify:

```rust
// Demo game's falling sand - users copy and modify
fn try_fall_and_slide(
    pos: WorldPos,
    pixels: &LayerRef<Pixel>,
    materials: &MaterialRegistry,
    params: FallParams,
) -> Option<WorldPos> {
    // Uses library helpers internally
    let below = WorldPos::new(pos.x, pos.y - 1);
    let dst = pixels.get(below)?;
    if can_displace(&pixels.get(pos)?, &dst, materials) {
        return Some(below);
    }
    // ... diagonal sliding logic using disperse()
    None
}
```

**Key insight:** Library provides building blocks (`disperse`, `can_displace`). Demo shows how to compose them. Users copy demo code into their games and customize.

### Hash Functions

Deterministic hashing for simulation randomness (`simulation/hash.rs`):

```rust
/// 2 inputs → 1 output
pub fn hash21uu64(a: u64, b: u64) -> u64

/// 4 inputs → 1 output
pub fn hash41uu64(a: u64, b: u64, c: u64, d: u64) -> u64
```

| Property | Guarantee |
|----------|-----------|
| Deterministic | Same inputs always produce same output |
| Well-distributed | Adjacent values produce uncorrelated outputs |
| Fast | FNV-1a style mixing, no heap allocation |

**Common patterns:**

```rust
// Per-pixel randomness (position + tick)
let h = hash21uu64(pos.x as u64, pos.y as u64);
let direction = h % 2;  // left or right

// Per-pixel + per-tick randomness
let h = hash41uu64(pos.x as u64, pos.y as u64, tick, 0);
let chance = (h % 100) < 30;  // 30% probability
```

### `disperse(dispersion, tick, pos) -> (i64, i64)`

Parametric dispersion offset for gases and liquids.

| Parameter | Effect |
|-----------|--------|
| `dispersion: 0` | No horizontal spread |
| `dispersion: 1-3` | Slow spread (honey, mud) |
| `dispersion: 4-7` | Medium spread (water) |
| `dispersion: 8+` | Fast spread (gas, air) |

Uses deterministic hashing for consistent random direction selection.

### `can_displace(src, dst, materials) -> bool`

Density-based displacement check.

| Comparison | Result |
|------------|--------|
| src denser than dst | Can displace (heavier sinks) |
| src lighter than dst | Cannot displace |
| src is void | Cannot displace |
| dst is void | Can displace |

### `neighbors_matching(pos, get_pixel) -> u8`

Count neighbors matching a condition.

```rust
// Count adjacent water pixels
let water_count = neighbors_matching(pos, |p| {
    pixels.get(p).map(|px| px.material == WATER)
});

// Count burning neighbors
let fire_count = neighbors_matching(pos, |p| {
    pixels.get(p).map(|px| px.flags.contains(PixelFlags::BURNING))
});
```

Returns count 0-8 for the Moore neighborhood.

### `raycast(origin, direction, get_pixel, max_dist) -> RaycastHit`

Pixel-level raycasting.

| Field | Type | Description |
|-------|------|-------------|
| `hit_pos` | `Option<WorldPos>` | First non-void pixel hit |
| `distance` | `f32` | Distance traveled |
| `normal` | `(i8, i8)` | Surface normal at hit point |

**Use cases:**

- Line-of-sight checks
- Projectile collision
- Light propagation

## Demo Simulation Pattern

A complete demo simulation showing how to compose library functions:

```rust
// Demo game's physics system (users copy and customize)
fn falling_sand_sim(
    iter: PhasedIter<Pixel>,
    mut pixels: LayerMut<Pixel>,
    materials: Res<MaterialRegistry>,
    ctx: Res<SimContext>,  // separate resource: tick, seed, jitter
) {
    iter.for_each(|frag| {
        let pos = frag.pos();
        let pixel = pixels.get(pos);
        if pixel.is_void() { return; }

        let material = materials.get(pixel.material);

        // Demo-specific physics logic
        let target = match material.state {
            PhysicsState::Solid => None,
            PhysicsState::Powder => {
                // Try falling with drift
                let (dx, dy) = disperse(material.dispersion, ctx.tick, pos);
                let below = WorldPos::new(pos.x + dx, pos.y - 1);
                if can_displace(&pixel, &pixels.get(below)?, &materials) {
                    Some(below)
                } else {
                    // Try diagonal...
                    None
                }
            }
            PhysicsState::Liquid => { /* similar */ None }
            PhysicsState::Gas => None,
        };

        if let Some(target) = target {
            pixels.swap(pos, target);
            // ColorLayer, DamageLayer swap automatically (swap-follow)
        }
    });
}
```

## Composition Patterns

### Layering Custom Behavior

Delegate to library functions for common physics:

```rust
fn magnetic_sim(
    iter: PhasedIter<Pixel>,
    mut pixels: LayerMut<Pixel>,
    materials: Res<MaterialRegistry>,
) {
    iter.for_each(|frag| {
        let pos = frag.pos();
        let pixel = pixels.get(pos);

        // Custom behavior: magnetic pixels attracted to iron
        if pixel.material == MAGNETIC {
            if let Some(iron_pos) = find_nearby_iron(pos, &pixels) {
                move_toward(pos, iron_pos, &mut pixels);
                return;
            }
        }

        // Fall back to standard physics (demo's try_fall_and_slide)
        if let Some(target) = try_fall_and_slide(pos, &pixels, &materials) {
            pixels.swap(pos, target);
        }
    });
}
```

### Multi-Layer Access

Read/write multiple layers in one system:

```rust
fn thermal_rising_sim(
    iter: PhasedIter<Pixel>,
    mut pixels: LayerMut<Pixel>,
    temperature: Res<TemperatureLayer>,  // read-only, swap-follow layer
) {
    iter.for_each(|frag| {
        let pos = frag.pos();
        let temp = temperature.get(pos);
        let pixel = pixels.get(pos);

        // Hot pixels rise faster
        if temp > 200 && pixel.material == STEAM {
            if let Some(target) = try_rise_fast(pos, &pixels) {
                pixels.swap(pos, target);
                // TemperatureLayer swaps automatically (swap-follow)
            }
        }
    });
}
```

### Conditional Material Behavior

Override behavior for specific materials:

```rust
fn custom_materials_sim(
    iter: PhasedIter<Pixel>,
    mut pixels: LayerMut<Pixel>,
    materials: Res<MaterialRegistry>,
) {
    iter.for_each(|frag| {
        let pos = frag.pos();
        match pixels.get(pos).material {
            CUSTOM_SLIME => slime_behavior(pos, &mut pixels),
            CUSTOM_METAL => metal_behavior(pos, &mut pixels),
            _ => {
                if let Some(target) = try_fall_and_slide(pos, &pixels, &materials) {
                    pixels.swap(pos, target);
                }
            }
        }
    });
}
```

## Performance Notes

### Dirty Rect Tracking

The biggest win: dirty rect tracking skips ~90% of tiles under typical workloads. Only actively changing regions are simulated.

### General

| Aspect | Consideration |
|--------|---------------|
| Inlining | Library functions are `#[inline]` for zero-cost abstraction |
| Branching | Match on material ID is fast (single u8 comparison) |
| Swap-follow | 2-3 layers typical; loop overhead minimal |
| RNG | `hash21uu64` is faster than thread-local RNG for spatial randomness |

### Cache Locality

Each tile (32×32 = 1024 pixels) fits in L1 cache:
- Pixel: 2 KB
- ColorLayer: 1 KB
- DamageLayer: 1 KB
- **FallingSandBundle: ~4 KB** (L1 is typically 32-64 KB)

## Related Documentation

- [Simulation](../simulation/simulation.md) - Core simulation passes and scheduling
- [Scheduling](../simulation/scheduling.md) - Parallel execution infrastructure and schedule modes
- [Materials](../simulation/materials.md) - Material properties used by library functions
- [Pixel Layers](pixel-layers.md) - Extension layers for custom state
- [Pixel Format](../foundational/pixel-format.md) - Base pixel data accessed by rules
- [Architecture Overview](../README.md)
