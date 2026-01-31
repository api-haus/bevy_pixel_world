# Pixel Layers

Modular layer system where every piece of per-pixel data is an opt-in layer.

## Core Concept

The only mandatory data per pixel is the **Material ID** (1 byte). Everything else—color, damage, flags, temperature—is an optional layer that simulations opt into.

```
Base Layer (always present):
  Material: u8  // 1 byte, indexes material registry

Default Bundle Layers (opt-in, included in preset):
  Color: u8     // palette index
  Damage: u8    // accumulated damage
  Flags: u8     // dirty, solid, falling, burning, wet, pixel_body

Additional Layers (opt-in):
  Temperature, Velocity, Heat, Pressure, etc.
```

## Layer Bundles

Bundles are presets that register common layer combinations:

| Bundle | Layers | Use Case |
|--------|--------|----------|
| **Minimal** | Material only | Maximum performance, custom simulation |
| **Default** | Material + Color + Damage + Flags | Standard falling sand (backward compatible) |
| **Custom** | Builder API | Game-specific combinations |

### Builder API

```
PixelWorldPlugin::builder()
    .with_bundle(DefaultBundle)  // or MinimalBundle
    .with_layer::<HeatLayer>()
    .with_layer::<TemperatureLayer>()
    .with_simulation::<FallingSandSim>()
    .with_simulation::<HeatDiffusionSim>()
    .build()
```

### Default Bundle

The Default Bundle provides backward-compatible behavior matching the current "4-byte pixel" model:

| Layer | Purpose | Required By |
|-------|---------|-------------|
| Material | Type ID, always present | All simulations |
| Color | Palette index for rendering | Rendering systems |
| Damage | Accumulated damage | Destruction, decay |
| Flags | Simulation state bits | CA physics, collision |

## Base Layer (Innate)

Every chunk has a hardcoded base layer containing material IDs. This is not opt-in - it's fundamental to the simulation.

```
base: [MaterialId; CHUNK_SIZE²]  // u8, always present
```

The layer system described below is for *additional* data on top of this base.

## Optional Layers

| Layer | Type | Sample Rate | Purpose |
|-------|------|-------------|---------|
| Color | u8 | 1 | Palette index |
| Damage | u8 | 1 | Accumulated damage |
| Flags | u8 | 1 | Simulation state bits |
| Temperature | u8 | 1 | Per-pixel temperature |
| Velocity | (i8, i8) | 1 | Pixel momentum |
| Heat | u8 | 4 | Thermal diffusion (downsampled) |
| Pressure | u16 | 8 | Fluid/gas pressure (downsampled) |

## Brick Layer (Demo)

A reference implementation for block-based destruction gameplay, included in the demo game. Copy it into your game and adapt as needed.

### Concept

Bricks subdivide chunks into destructible blocks. Players hit pixels, damage accumulates per-brick, and when threshold is exceeded, all pixels in that brick are destroyed.

```
BrickLayer<const GRID: usize = 16> {
    // Full resolution - which brick each pixel belongs to
    id: [BrickId; CHUNK_SIZE²],

    // Downsampled - one damage value per brick
    damage: [u8; GRID²],
}
```

The const generic `GRID` controls everything:

| GRID | Bricks | Brick Pixels | Id Type | Damage Cells |
|------|--------|--------------|---------|--------------|
| 16 | 256 | CHUNK_SIZE/16 | u8 | 256 |
| 32 | 1024 | CHUNK_SIZE/32 | u16 | 1024 |
| 64 | 4096 | CHUNK_SIZE/64 | u16 | 4096 |

**BrickId type derivation:**
- `GRID² ≤ 256` → `u8`
- `GRID² > 256` → `u16`

### Sample Rates

| Sub-layer | Sample Rate | Formula |
|-----------|-------------|---------|
| `id` | 1 | Full resolution (pixel → brick mapping) |
| `damage` | CHUNK_SIZE / GRID | One cell per brick |

For a 512×512 chunk with `GRID = 16`:
- Brick pixel size: 512 / 16 = 32×32 pixels per brick
- Damage sample rate: 32 (matches brick size)
- 256 bricks, 256 damage cells

### Usage

Since both CHUNK_SIZE and GRID are compile-time constants, a macro generates the layer types:

```
define_brick_layer!(GameBrickLayer, chunk_size: 512, grid: 16);
// Generates BrickIdLayer + BrickDamageLayer with correct types and sizes

// During plugin init:
let brick_handles = GameBrickLayer::register(&mut layer_registry);
commands.insert_resource(brick_handles);

// In systems:
fn brick_damage_system(
    mut chunks: Query<&mut Chunk>,
    brick: Res<BrickLayerHandles>,
) {
    for mut chunk in &mut chunks {
        let ids = chunk.get(brick.id);
        let damage = chunk.get_mut(brick.damage);
        // accumulate damage per brick...
    }
}
```

### Gameplay Flow

```mermaid
sequenceDiagram
    participant Player
    participant Pixel as Pixel Layer
    participant Brick as BrickLayer

    Player->>Pixel: Hit pixel at (x, y)
    Pixel->>Brick: Lookup brick_id[x, y]
    Brick-->>Brick: damage[brick_id] += hit_damage
    alt damage >= threshold
        Brick->>Pixel: Destroy all pixels where brick_id == damaged_brick
    end
```

### GPU Upload

Both sub-layers are uploaded for shader-based damage visualization:

| Sub-layer | Schedule | Shader Use |
|-----------|----------|------------|
| `id` | `OnChange` | Map pixel to brick for effect lookup |
| `damage` | `Periodic(4)` | Damage overlay (cracks, glow) on whole brick |

The shader combines both: sample `id` to find which brick, sample `damage` to determine visual intensity. Entire brick regions show damage effects uniformly.

### Customization

`BrickLayer` is a reference implementation. For different needs:

- **Different damage type**: Use macro with custom damage type for more granularity
- **Multiple damage types**: Add `fire_damage`, `physical_damage` sub-layers
- **Non-uniform bricks**: Replace uniform grid with runtime-defined brick shapes (loses const-generic benefits)

## Simulation Systems

Each simulation declares which layers it requires and writes:

```
trait SimulationRule {
    /// Layers this simulation reads
    fn required_layers() -> &'static [LayerId];

    /// Layers this simulation writes
    fn writes_layers() -> &'static [LayerId];

    /// Compute movement for a single pixel
    fn compute_swap(...) -> Option<WorldPos>;
}
```

### Scheduling

- **Missing layer = system skipped** (configurable: skip silently or panic)
- **Disjoint write sets = parallel execution** (Bevy scheduler handles this)
- **Shared write sets = sequential execution** (ordered by registration)

```mermaid
flowchart LR
    subgraph Parallel["Parallel (disjoint writes)"]
        A["Falling Sand<br/>writes: Flags"]
        B["Heat Diffusion<br/>writes: Heat"]
    end
    subgraph Sequential["Sequential (shared writes)"]
        C["Material Interactions<br/>writes: Material, Damage"]
        D["Decay Pass<br/>writes: Material, Damage"]
    end
    Parallel --> Sequential
```

## Overview

All auxiliary data uses the same layer abstraction. The **sample rate** parameter determines resolution and available features:

| Sample Rate | Resolution | Cells per Chunk (512×512) | Swap-Follow | Use Case |
|-------------|------------|---------------------------|-------------|----------|
| 1 | 1:1 with pixels | 262,144 | Available | Temperature, velocity, age |
| 4 | 4×4 pixels per cell | 16,384 | N/A | Heat map, moisture zones |
| 8 | 8×8 pixels per cell | 4,096 | N/A | Pressure regions, light |

## Architecture

```mermaid
flowchart TB
    subgraph Chunk["Chunk Buffer"]
        direction TB
        Base["Base Layer<br/>Material (u8)<br/>always present"]
        L0["Layer: Color<br/>sample_rate: 1<br/>(opt-in, Default Bundle)"]
        L1["Layer: Damage<br/>sample_rate: 1<br/>(opt-in, Default Bundle)"]
        L2["Layer: Flags<br/>sample_rate: 1<br/>(opt-in, Default Bundle)"]
        L3["Layer: Heat<br/>sample_rate: 4<br/>(opt-in)"]
        L4["Layer: Pressure<br/>sample_rate: 8<br/>(opt-in)"]
    end

    subgraph Render["Render Pipeline"]
        direction LR
        R1["GPU Upload<br/>(per-layer schedule)"]
        R2["Backend<br/>(per-layer)"]
    end

    L0 --> R1
    L3 --> R1
    L4 --> R1
    R1 --> R2
```

## Layer Definition

Each layer declares its properties at registration:

```
trait Layer {
    /// Element type stored in this layer
    type Element: Copy + Default;

    /// Pixels per cell (1 = full resolution, 4 = 4×4, etc.)
    const SAMPLE_RATE: u32;

    /// Layer name for debugging and serialization
    const NAME: &'static str;
}
```

### Sample Rate

Determines the resolution ratio between pixels and layer cells:

| Sample Rate | Meaning | Memory Reduction |
|-------------|---------|------------------|
| 1 | One cell per pixel | None (full resolution) |
| 2 | One cell per 2×2 pixels | 4× |
| 4 | One cell per 4×4 pixels | 16× |
| 8 | One cell per 8×8 pixels | 64× |

**Coordinate mapping:**

```
layer_x = pixel_x / sample_rate
layer_y = pixel_y / sample_rate
```

## Base Layer

The base layer contains only the Material ID with `sample_rate: 1`:

| Field | Type | Purpose |
|-------|------|---------|
| Material | u8 | Type identifier, indexes into material registry |

**Total: 1 byte per pixel (minimum)**

Additional fields (Color, Damage, Flags) are opt-in layers included in the Default Bundle.

See [Pixel Format](../foundational/pixel-format.md) for the base layer specification and flag bitmask reference.

### Stability Guarantee

The base layer (Material only) will not change within a major version. The Default Bundle layers (Color, Damage, Flags) are stable for backward compatibility.

## Swap-Follow

Layers with `sample_rate: 1` can opt into synchronized swapping with the base layer.

### Behavior

When enabled (default for `sample_rate: 1`):

```mermaid
sequenceDiagram
    participant Sim as Simulation
    participant Base as Base Layer
    participant Temp as Temperature (swap_follow: true)
    participant Vel as Velocity (swap_follow: true)

    Sim->>Base: swap(pos_a, pos_b)
    Base-->>Sim: pixels swapped
    Sim->>Temp: swap(pos_a, pos_b)
    Sim->>Vel: swap(pos_a, pos_b)
```

### Configuration

```
struct LayerConfig {
    /// Follow base layer swaps (only valid when sample_rate = 1)
    swap_follow: bool,  // default: true

    /// Save to disk or resimulate on load
    persistent: bool,   // default: false
}
```

| Sample Rate | swap_follow | Behavior |
|-------------|-------------|----------|
| 1 | true (default) | Layer data moves with pixels |
| 1 | false | Layer data stays at position (spatial field) |
| > 1 | N/A | Coarse resolution, no pixel correspondence |

### Use Cases

**swap_follow: true** (default)
- Temperature that belongs to the pixel (hot lava stays hot when falling)
- Velocity/momentum (pixel carries its motion)
- Age (pixel's lifetime counter)

**swap_follow: false**
- Spatial fields (wind direction at a location)
- Environmental zones (radiation level at position)

## Downsampled Layers

Layers with `sample_rate > 1` store coarse data that applies to pixel regions:

### Heat Layer Example

```
struct HeatLayer;

impl Layer for HeatLayer {
    type Element = u8;  // 0-255 temperature
    const SAMPLE_RATE: u32 = 4;
    const NAME: &'static str = "heat";
}
```

**Properties:**
- One heat cell per 4×4 pixel region
- 16× memory reduction vs full resolution
- Smooth gradients more physically plausible than per-pixel heat

### Aggregation

Downsampled layers aggregate from pixels or propagate between cells:

```
// Heat accumulation from burning pixels
for pixel in cell_region {
    if pixel.flags.burning {
        cell.heat += BURN_HEAT;
    }
}

// Diffusion between cells
new_heat = (self + neighbors.avg()) / 2 * cooling_factor;
```

## Render Modularity

Each layer controls its own GPU upload pipeline.

### Scheduling Model

All upload schedules run **after** the pixel simulation group completes:

```mermaid
flowchart LR
    subgraph SimGroup["Simulation Group"]
        CA["Cell Simulation<br/>(4 phases)"]
        P["Particles"]
        MI["Material Interactions"]
        CA --> P --> MI
    end

    subgraph Upload["Upload Schedules"]
        U1["Base Layer Upload"]
        U2["Heat Layer Upload"]
        U3["Custom Layer Upload"]
    end

    SimGroup --> Upload
```

### Default Schedule

The default upload schedule:

1. Runs after each simulation group tick
2. Checks chunk dirty flag
3. Uploads only if dirty

```
struct DefaultUploadSchedule;

impl UploadSchedule for DefaultUploadSchedule {
    fn should_upload(&self, chunk: &Chunk, tick: u64) -> bool {
        chunk.is_dirty()
    }

    fn tick_divisor(&self) -> u32 {
        1  // every simulation tick
    }
}
```

### Custom Schedules

Custom schedules can modify both the check logic and tick rate:

```
trait UploadSchedule {
    /// Custom condition for upload (default: dirty check)
    fn should_upload(&self, chunk: &Chunk, tick: u64) -> bool;

    /// Run every N simulation ticks (1 = every tick, 4 = every 4th tick)
    fn tick_divisor(&self) -> u32;
}
```

### Schedule Presets

| Preset | `tick_divisor` | Check | Use Case |
|--------|----------------|-------|----------|
| `OnChange` | 1 | Dirty flag | Base pixels - immediate visual feedback |
| `Periodic(n)` | n | Always true | Heat - interpolation hides latency |
| `OnChangeThrottled(n)` | n | Dirty flag | Large layers - reduce upload frequency |
| `Never` | - | Always false | Velocity - simulation-only, not rendered |

### Examples

| Layer | Schedule | Behavior |
|-------|----------|----------|
| Base pixels | `OnChange` | Upload every tick if any pixel changed |
| Heat | `Periodic(4)` | Upload every 4th tick unconditionally |
| Moisture | `OnChangeThrottled(2)` | Upload every 2nd tick if dirty |
| Velocity | `Never` | No GPU upload, CPU-only |

### Backend Integration

Layers provide render data through the `LayerRender` trait:

```
trait LayerRender {
    /// Upload schedule for this layer
    fn schedule(&self) -> &dyn UploadSchedule;

    /// Called when schedule triggers upload
    fn upload(&self, gpu: &mut GpuContext);

    /// Shader uniform binding (if any)
    fn binding(&self) -> Option<BindGroup>;
}
```

**Shader integration examples:**
- Heat layer → uniform buffer for glow tinting
- Moisture layer → wet sheen intensity multiplier
- Custom layer → game-specific visual effects

## Persistence

Layers are either **persistent** (saved to disk) or **transient** (resimulated on load).

### Persistent Layers

Saved alongside base pixel data in chunk files:

| Property | Behavior |
|----------|----------|
| Serialization | Binary format, streamed with chunk |
| Load | Read from disk, ready immediately |
| Use case | Source-of-truth data that can't be derived |

**Examples:**
- Base pixel layer (always persistent)
- Player-placed markers or ownership data
- Light/visibility (fog of war - explored areas stay revealed)
- Accumulated damage that affects gameplay

### Transient Layers

Not saved; regenerated when chunk loads:

| Property | Behavior |
|----------|----------|
| Serialization | None |
| Load | Initialized to default, resimulated |
| Use case | Derived/computed data |

**Examples:**
- Heat (derived from burning pixels, diffuses from neighbors)
- Velocity cache (derived from recent movement)
- Collision cache (derived from solid pixels)

### Configuration

```
// Persistent: saved to disk
world.register_layer::<OwnershipLayer>(LayerConfig {
    persistent: true,
    ..default()
});

// Transient: resimulated on load (default)
world.register_layer::<HeatLayer>(LayerConfig {
    persistent: false,  // default
    ..default()
});
```

### Chunk File Format

Persistent layers append to chunk save format:

```
ChunkFile:
  header: ChunkHeader
  base_pixels: [Pixel; CHUNK_SIZE²]
  layer_ownership: [u8; CHUNK_SIZE²]    // if registered + persistent
  layer_custom: [T; cells]              // if registered + persistent
```

See [Chunk Persistence](../persistence/chunk-persistence.md) for save format details.

## Memory Layout

All layers use SoA (Structure of Arrays) for cache efficiency:

```
Material:     [M0, M1, M2, M3, M4, ...]     // 1 byte each,  262k cells (always)
Color:        [C0, C1, C2, C3, C4, ...]     // 1 byte each,  262k cells (Default Bundle)
Damage:       [D0, D1, D2, D3, D4, ...]     // 1 byte each,  262k cells (Default Bundle)
Flags:        [F0, F1, F2, F3, F4, ...]     // 1 byte each,  262k cells (Default Bundle)
Heat:         [H0, H1, H2, H3, ...]         // 1 byte each,  16k cells  (opt-in)
Pressure:     [R0, R1, R2, ...]             // 2 bytes each, 4k cells   (opt-in)
```

### Memory Examples

For a 512×512 chunk with different configurations:

**Minimal Bundle (Material only):**

| Layer | Sample Rate | Cells | Per-Cell | Total |
|-------|-------------|-------|----------|-------|
| Material | 1 | 262,144 | 1 byte | 256 KB |
| **Total** | | | | **256 KB** |

**Default Bundle (backward compatible):**

| Layer | Sample Rate | Cells | Per-Cell | Total |
|-------|-------------|-------|----------|-------|
| Material | 1 | 262,144 | 1 byte | 256 KB |
| Color | 1 | 262,144 | 1 byte | 256 KB |
| Damage | 1 | 262,144 | 1 byte | 256 KB |
| Flags | 1 | 262,144 | 1 byte | 256 KB |
| **Total** | | | | **1 MB** |

**Default Bundle + Heat + Pressure:**

| Layer | Sample Rate | Cells | Per-Cell | Total |
|-------|-------------|-------|----------|-------|
| Material | 1 | 262,144 | 1 byte | 256 KB |
| Color | 1 | 262,144 | 1 byte | 256 KB |
| Damage | 1 | 262,144 | 1 byte | 256 KB |
| Flags | 1 | 262,144 | 1 byte | 256 KB |
| Heat | 4 | 16,384 | 1 byte | 16 KB |
| Pressure | 8 | 4,096 | 2 bytes | 8 KB |
| **Total** | | | | **~1 MB** |

## Registration

Layers are registered at startup:

```
world.register_layer::<TemperatureLayer>(LayerConfig {
    swap_follow: true,
    persistent: true,  // save pixel temperatures
});

world.register_layer::<HeatLayer>(LayerConfig::default());  // transient, resimulates
```

### Requirements

| Requirement | Rationale |
|-------------|-----------|
| Register before first chunk | Ensures all chunks have consistent layers |
| Fixed set per session | Dynamic registration would complicate sync |
| Declare sample rate at compile time | Enables static allocation sizing |

## Related Documentation

- [Pixel Format](../foundational/pixel-format.md) - Base layer specification
- [Simulation](../simulation/simulation.md) - Heat layer propagation, swap mechanics
- [Simulation Extensibility](simulation-extensibility.md) - Custom rules using layer data
- [Rendering Backends](rendering-backends.md) - Backend integration for layer rendering
- [Chunk Pooling](../chunk-management/chunk-pooling.md) - Chunk lifecycle and layer allocation
- [Chunk Persistence](../persistence/chunk-persistence.md) - Save format for persistent layers
- [Architecture Overview](../README.md)
