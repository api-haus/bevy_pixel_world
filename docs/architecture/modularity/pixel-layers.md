# Pixel Layers

> **Status: Planned Architecture**
>
> Current implementation uses a monolithic 4-byte `Pixel` struct. The layer system described here is not yet implemented.

Layer system for pixel data at varying resolutions.

## Core Concept

Two storage patterns:

1. **Pixel struct (AoS):** Main pixel data, swaps atomically during simulation
2. **Separate layers (SoA):** Per-layer arrays with independent resolution and lifetime

## Sample Rate

Determines resolution ratio between pixels and layer cells:

| Sample Rate | Meaning | Memory vs Full |
|-------------|---------|----------------|
| 1 | One cell per pixel | 1× |
| 4 | One cell per 4×4 pixels | 16× smaller |
| 8 | One cell per 8×8 pixels | 64× smaller |

**Coordinate mapping:** `layer_coord = pixel_coord / sample_rate`

## Swap-Follow

Layers with `sample_rate: 1` can follow pixel swaps or stay spatial:

| swap_follow | Behavior | Use Case |
|-------------|----------|----------|
| true | Data moves with pixel | Temperature, velocity, age |
| false | Data stays at location | Wind direction, radiation zones |

### Temperature vs Heat Example

| Layer | Sample Rate | swap_follow | Meaning |
|-------|-------------|-------------|---------|
| Temperature | 1 | true | "This pixel is hot" |
| Heat | 4 | false | "This location is hot" |

**Temperature:** Lava pixel has temperature=255. When it falls through a cold cave, temperature stays 255 - the lava is inherently hot.

**Heat:** Cave has a heat grid at 1/4 resolution. Lava passing through adds heat to that location. After lava falls away, heat persists and slowly diffuses. A flammable pixel entering this region might ignite from ambient heat.

## Persistence

| Type | Saved | Use Case |
|------|-------|----------|
| Persistent | Yes | Player-placed markers, fog of war, accumulated damage |
| Transient | No | Heat (derived from burning), velocity cache, collision cache |

Transient layers reinitialize to default on chunk load and resimulate.

## Planned Layers

| Layer | Type | Sample Rate | swap_follow | Persistent |
|-------|------|-------------|-------------|------------|
| Temperature | u8 | 1 | true | false |
| Velocity | (i8, i8) | 1 | true | false |
| Heat | u8 | 4 | false | false |
| Pressure | u16 | 8 | false | false |

## Memory Layout

For a 512×512 chunk with 4-byte pixels:

| Storage | Cells | Size |
|---------|-------|------|
| Pixel array | 262,144 | 1 MB |
| Heat (rate 4) | 16,384 | 16 KB |
| Pressure (rate 8) | 4,096 | 8 KB |

## Related Documentation

- [Pixel Format](../foundational/pixel-format.md) - Current pixel structure
- [Simulation](../simulation/simulation.md) - Swap mechanics
- [Chunk Persistence](../persistence/chunk-persistence.md) - Save format
