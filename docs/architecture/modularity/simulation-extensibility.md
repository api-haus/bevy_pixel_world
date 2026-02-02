# Simulation Extensibility

> **Status: Planned Architecture**
>
> Describes simulation patterns. Current implementation has hardcoded simulation passes.

Simulation rules and reusable functions.

## Core Model

A simulation is a Bevy system combined with a schedule mode:

```
Simulation = Bevy System + ScheduleMode
```

**Bevy System:** Function parameters declare layer access. Bevy handles inter-system parallelism based on disjoint writes.

**Schedule Mode:** How pixels are iterated within the system:
- `PhasedParallel`: Checkerboard 4-phase (spatial isolation, safe for local ops)
- `Sequential`: One pixel at a time (for global state access)

## Two Levels of Parallelism

| Level | Mechanism | Safety |
|-------|-----------|--------|
| Between systems | Bevy scheduler (disjoint layer writes → parallel) | Automatic |
| Within system | Schedule mode (how pixels iterated) | Mode-dependent |

**Example:**
```
FallingSandSim: writes Pixel     → PhasedParallel  ┐
HeatDiffusionSim: writes Heat    → PhasedParallel  ┘ Run in parallel

DecaySim: writes Pixel           → Sequential  ┐
InteractionSim: writes Pixel     → Sequential  ┘ Run sequentially
```

## Schedule Modes

| Mode | Mechanism | Use Case |
|------|-----------|----------|
| `PhasedIter` | Checkerboard 4-phase, barrier between phases | Standard CA physics |
| Sequential | Regular iteration, single-threaded | Global state, complex deps |

See [Scheduling](../simulation/scheduling.md) for checkerboard mechanics.

## Shared Functions

Reusable building blocks for simulation rules.

### Hash Functions

Deterministic hashing for simulation randomness:

```rust
pub fn hash21uu64(a: u64, b: u64) -> u64
pub fn hash41uu64(a: u64, b: u64, c: u64, d: u64) -> u64
```

| Property | Guarantee |
|----------|-----------|
| Deterministic | Same inputs → same output |
| Well-distributed | Adjacent values → uncorrelated outputs |
| Fast | FNV-1a style, no heap allocation |

### disperse

```rust
pub fn disperse(dispersion: u8, tick: u64, pos: WorldPos) -> (i64, i64)
```

Parametric dispersion offset for gases and liquids.

| dispersion | Spread |
|------------|--------|
| 0 | None |
| 1-3 | Slow (honey, mud) |
| 4-7 | Medium (water) |
| 8+ | Fast (gas, air) |

### can_displace

```rust
pub fn can_displace(src: &Pixel, dst: &Pixel, materials: &MaterialRegistry) -> bool
```

Density-based displacement: heavier sinks, lighter floats.

### neighbors_matching

```rust
pub fn neighbors_matching<F>(pos: WorldPos, predicate: F) -> u8
```

Count neighbors (0-8) matching a condition in Moore neighborhood.

### raycast

```rust
pub fn raycast(origin: WorldPos, direction: (f32, f32), get_pixel: F, max_dist: f32) -> RaycastHit
```

Pixel-level raycasting for line-of-sight, projectiles, light propagation.

## Performance Notes

### Dirty Rect Tracking

Skips ~90% of tiles under typical workloads. Only actively changing regions simulate.

### Cache Locality

Each tile (32×32 = 1024 pixels) fits in L1 cache:
- 4-byte pixel: 4 KB per tile
- L1 is typically 32-64 KB

### General

| Aspect | Note |
|--------|------|
| Inlining | Shared functions are `#[inline]` |
| Branching | Material ID match is single u8 comparison |
| Swap-follow | 2-3 layers typical, minimal overhead |
| RNG | `hash21uu64` faster than thread-local RNG |

## Related Documentation

- [Simulation](../simulation/simulation.md) - Core simulation passes
- [Scheduling](../simulation/scheduling.md) - Checkerboard mechanics
- [Materials](../simulation/materials.md) - Material properties
- [Pixel Layers](pixel-layers.md) - Layer system
