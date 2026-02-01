# Pixel Layers

> **Status: Planned Architecture**
>
> This document describes a planned layer system. Current implementation uses a monolithic 4-byte `Pixel` struct (material, color, damage, flags). The macros and generic infrastructure described here are not yet implemented.

Modular layer system where the **game defines its own pixel structure**.

## Core Concept

**Radical modularity:** The framework has minimal opinions about pixel contents.

- Framework provides: `Chunk<T>`, `Canvas<T>`, iteration primitives
- Framework requires: `T: Copy + Default + 'static` (that's it)
- Game defines: pixel struct with whatever fields it needs

### Optional Traits

Framework features that need pixel information use optional traits:

```rust
/// For collision mesh generation (marching squares)
pub trait PixelCollision {
    fn is_solid(&self) -> bool;
}

/// For dirty-based simulation scheduling
pub trait PixelDirty {
    fn is_dirty(&self) -> bool;
    fn set_dirty(&mut self, dirty: bool);
}
```

**Implement what you need:**
- Want collision meshes? Implement `PixelCollision`
- Want dirty-based scheduling? Implement `PixelDirty`
- Don't need them? Don't implement them

```rust
// Game defines this however it wants
struct Pixel {
    material: u8,
    color: u8,
    damage: u8,
    flags: MyFlags,
}

// Optional: for collision
impl PixelCollision for Pixel {
    fn is_solid(&self) -> bool { self.flags.contains(MyFlags::SOLID) }
}

// Optional: for scheduling
impl PixelDirty for Pixel {
    fn is_dirty(&self) -> bool { self.flags.contains(MyFlags::DIRTY) }
    fn set_dirty(&mut self, v: bool) { self.flags.set(MyFlags::DIRTY, v); }
}
```

## Demo Game as Reference

The demo game shows one way to structure pixels. Users clone and modify.

```
Demo Game Pixel (4 bytes):
  Material: u8    // indexes game's material registry
  Color: u8       // palette index for rendering
  Damage: u8      // accumulated damage
  Flags: u8       // dirty, solid, falling, burning, etc.

Your Game Pixel (whatever you need):
  // Define your own fields, your own meaning
```

This is not a "bundle system" with presets. It's "here's how we did it, adapt to your needs."

## Separate Layers (SoA)

Beyond the pixel struct (AoS), games can register separate layers stored as Structure-of-Arrays:

| Layer | Type | Sample Rate | Purpose |
|-------|------|-------------|---------|
| Temperature | u8 | 1 | Per-pixel temperature (swaps with pixel) |
| Velocity | (i8, i8) | 1 | Pixel momentum (swaps with pixel) |
| Heat | u8 | 4 | Thermal diffusion (downsampled, spatial) |
| Pressure | u16 | 8 | Fluid/gas pressure (downsampled, spatial) |

These are defined and registered by the game, not the framework.

## Brick Layer (Demo Game Example)

A reference implementation for block-based destruction gameplay, included in the demo game. Copy and adapt as needed.

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

Simulations are implemented by the game, not the framework.

The framework provides:
- Iteration primitives (checkerboard phasing)
- Chunk dirty tracking
- Bevy system scheduling infrastructure

The game implements:
- All simulation rules (falling, burning, spreading)
- Material interactions
- Heat diffusion
- Any game-specific behavior

```mermaid
flowchart LR
    subgraph Framework["Framework (bevy_pixel_world)"]
        A["Iteration Primitives"]
        B["Dirty Tracking"]
    end
    subgraph Game["Game Crate"]
        C["Falling Sand Sim"]
        D["Heat Diffusion Sim"]
        E["Material Interactions"]
    end
    A --> C
    A --> D
    B --> E
```

## Overview

Two storage patterns:

1. **Pixel struct (AoS):** Game-defined struct, swaps atomically
2. **Separate layers (SoA):** Per-layer arrays, independent lifetime

The **sample rate** parameter determines resolution for separate layers:

| Sample Rate | Resolution | Cells per Chunk (512×512) | Swap-Follow | Use Case |
|-------------|------------|---------------------------|-------------|----------|
| 1 | 1:1 with pixels | 262,144 | Available | Temperature, velocity, age |
| 4 | 4×4 pixels per cell | 16,384 | N/A | Heat map, moisture zones |
| 8 | 8×8 pixels per cell | 4,096 | N/A | Pressure regions, light |

## Architecture

```mermaid
flowchart TB
    subgraph Chunk["Chunk<GamePixel>"]
        direction TB
        Pixels["Pixel Array (AoS)<br/>Game-defined struct<br/>e.g. {material, color, damage, flags}"]
        L1["Layer: Heat<br/>sample_rate: 4<br/>(game-registered)"]
        L2["Layer: Pressure<br/>sample_rate: 8<br/>(game-registered)"]
    end

    subgraph Render["Render Pipeline"]
        direction LR
        R1["Raw Upload<br/>(shader interprets)"]
        R2["Backend + Palette LUT"]
    end

    Pixels --> R1
    L1 --> R1
    R1 --> R2
```

## Separate Layer Definition

For SoA layers registered separately from the pixel struct:

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

## Pixel Struct (Game-Defined)

The game defines its pixel struct. Framework doesn't know or care what's in it.

```rust
// Game crate defines this
#[repr(C)]
pub struct GamePixel {
    pub material: u8,  // game concept
    pub color: u8,     // game concept
    pub damage: u8,    // game concept
    pub flags: u8,     // game concept
}

// Framework just needs these bounds
impl Copy for GamePixel {}
impl Default for GamePixel { ... }
```

See the demo game for a reference implementation.

## Swap-Follow

Separate layers with `sample_rate: 1` can opt into synchronized swapping with the pixel array.

### Behavior

When enabled (default for `sample_rate: 1`):

```mermaid
sequenceDiagram
    participant Sim as Simulation
    participant Pixels as Pixel Array
    participant Temp as Temperature (swap_follow: true)
    participant Vel as Velocity (swap_follow: true)

    Sim->>Pixels: swap(pos_a, pos_b)
    Pixels-->>Sim: pixels swapped
    Sim->>Temp: swap(pos_a, pos_b)
    Sim->>Vel: swap(pos_a, pos_b)
```

### Configuration

```
struct LayerConfig {
    /// Follow pixel swaps (only valid when sample_rate = 1)
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

**swap_follow: true** (default) — Data belongs to the pixel:
- Temperature that belongs to the pixel (hot lava stays hot when falling)
- Velocity/momentum (pixel carries its motion)
- Age (pixel's lifetime counter)

**swap_follow: false** — Data belongs to the location:
- Spatial fields (wind direction at a location)
- Environmental zones (radiation level at position)

### Temperature vs Heat: A Clarifying Example

These two pixel layers sound similar but serve different purposes:

| Layer | Sample Rate | swap_follow | Semantic |
|-------|-------------|-------------|----------|
| Temperature | 1 | true | "This pixel is hot" |
| Heat | 4 | false | "This location is hot" |

**Temperature (sample_rate: 1, swap_follow: true):**
A lava pixel has temperature=255. When it falls through a cold cave, its temperature stays at 255 because the data moves with the pixel. The lava is inherently hot.

**Heat (sample_rate: 4, swap_follow: false):**
The cave has a heat grid at 1/4 resolution. When lava passes through, it *adds* heat to that location. After the lava falls away, the heat persists at that location, slowly diffusing to neighbors and cooling over time. A pixel entering this region reads the ambient heat and might ignite if flammable.

This distinction matters for gameplay:
- A bucket of water dumped on lava cools the *lava pixels* (reduces their Temperature)
- Lava flowing through a cave heats up the *cave region* (increases local Heat)
- An ice golem entering a heated region takes damage from ambient Heat, even if no hot pixels remain

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
        U1["Pixel Upload"]
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
| `OnChange` | 1 | Dirty flag | Pixels - immediate visual feedback |
| `Periodic(n)` | n | Always true | Heat - interpolation hides latency |
| `OnChangeThrottled(n)` | n | Dirty flag | Large layers - reduce upload frequency |
| `Never` | - | Always false | Velocity - simulation-only, not rendered |

### Examples

| Layer | Schedule | Behavior |
|-------|----------|----------|
| Pixels | `OnChange` | Upload every tick if any pixel changed |
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

Saved alongside pixel data in chunk files:

| Property | Behavior |
|----------|----------|
| Serialization | Binary format, streamed with chunk |
| Load | Read from disk, ready immediately |
| Use case | Source-of-truth data that can't be derived |

**Examples:**
- Pixel array (always persistent)
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
  pixels: [GamePixel; CHUNK_SIZE²]
  layer_ownership: [u8; CHUNK_SIZE²]    // if registered + persistent
  layer_custom: [T; cells]              // if registered + persistent
```

See [Chunk Persistence](../persistence/chunk-persistence.md) for save format details.

## Memory Layout

**Pixel struct (AoS):** Game-defined, stored contiguously:

```
Pixels: [P0, P1, P2, P3, ...]  // sizeof(GamePixel) each, 262k pixels
```

**Separate layers (SoA):** Registered by game, stored independently:

```
Heat:     [H0, H1, H2, H3, ...]  // 1 byte each, 16k cells (sample_rate: 4)
Pressure: [R0, R1, R2, ...]      // 2 bytes each, 4k cells (sample_rate: 8)
```

### Memory Examples

For a 512×512 chunk:

**4-byte pixel (demo game style):**

| Storage | Cells | Per-Cell | Total |
|---------|-------|----------|-------|
| GamePixel array | 262,144 | 4 bytes | 1 MB |
| **Total** | | | **1 MB** |

**4-byte pixel + heat + pressure:**

| Storage | Cells | Per-Cell | Total |
|---------|-------|----------|-------|
| GamePixel array | 262,144 | 4 bytes | 1 MB |
| Heat layer | 16,384 | 1 byte | 16 KB |
| Pressure layer | 4,096 | 2 bytes | 8 KB |
| **Total** | | | **~1 MB** |

**2-byte pixel (minimal):**

| Storage | Cells | Per-Cell | Total |
|---------|-------|----------|-------|
| MinimalPixel array | 262,144 | 2 bytes | 512 KB |
| **Total** | | | **512 KB** |

Games choose pixel size based on their needs. Smaller pixels = more chunks in memory.

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

## Bitpacking: Bring Your Own

The framework does **not** provide bitpacking macros. Games bring their own tools:

### Option 1: `bitflags!` (Recommended)

```rust
use bitflags::bitflags;

bitflags! {
    #[derive(Clone, Copy, Default)]
    pub struct PixelFlags: u8 {
        const DIRTY      = 0b0000_0001;
        const SOLID      = 0b0000_0010;
        const FALLING    = 0b0000_0100;
        const BURNING    = 0b0000_1000;
        const WET        = 0b0001_0000;
        const PIXEL_BODY = 0b0010_0000;
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GamePixel {
    pub material: u8,
    pub color: u8,
    pub damage: u8,
    pub flags: PixelFlags,
}

impl PixelCollision for GamePixel {
    fn is_solid(&self) -> bool { self.flags.contains(PixelFlags::SOLID) }
}

impl PixelDirty for GamePixel {
    fn is_dirty(&self) -> bool { self.flags.contains(PixelFlags::DIRTY) }
    fn set_dirty(&mut self, v: bool) { self.flags.set(PixelFlags::DIRTY, v); }
}
```

### Option 2: Manual Bit Manipulation

```rust
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GamePixel {
    pub material: u8,
    pub color: u8,
    pub damage_variant: u8,  // high nibble: damage, low nibble: variant
    pub flags: u8,
}

impl GamePixel {
    // Nibble access
    pub fn damage(&self) -> u8 { self.damage_variant >> 4 }
    pub fn variant(&self) -> u8 { self.damage_variant & 0x0F }
    pub fn set_damage(&mut self, v: u8) {
        self.damage_variant = (self.damage_variant & 0x0F) | (v << 4);
    }

    // Flag access
    pub fn burning(&self) -> bool { self.flags & 0x08 != 0 }
    pub fn set_burning(&mut self, v: bool) {
        if v { self.flags |= 0x08; } else { self.flags &= !0x08; }
    }
}

impl PixelCollision for GamePixel {
    fn is_solid(&self) -> bool { self.flags & 0x02 != 0 }
}

impl PixelDirty for GamePixel {
    fn is_dirty(&self) -> bool { self.flags & 0x01 != 0 }
    fn set_dirty(&mut self, v: bool) {
        if v { self.flags |= 0x01; } else { self.flags &= !0x01; }
    }
}
```

### Shader Access

Use `#[repr(C)]` for predictable memory layout:

```
┌──────────┬──────────┬──────────┬──────────┐
│ Byte 0   │ Byte 1   │ Byte 2   │ Byte 3   │
│ material │ color    │ dmg|var  │ flags    │
└──────────┴──────────┴──────────┴──────────┘
```

**WGSL shader access:**

```wgsl
let pixel_data: u32 = textureLoad(pixel_texture, coord, 0).r;
let material = pixel_data & 0xFFu;
let color = (pixel_data >> 8u) & 0xFFu;
let flags = (pixel_data >> 24u) & 0xFFu;

let is_burning = (flags & 0x08u) != 0u;
```

### Framework Integration

The framework is generic over `T: Copy + Default + 'static`:

```rust
pub struct Chunk<T: Copy + Default + 'static> {
    pixels: Surface<T>,
    // ...
}

pub struct PixelWorld<T: Copy + Default + 'static> {
    canvas: Canvas<T>,
    // ...
}
```

The game instantiates with its concrete type:

```rust
// In game crate
fn main() {
    App::new()
        .add_plugins(PixelWorldPlugin::<Pixel>::new(config))
        // ...
}
```

### Separate SoA Layers

Not all data belongs in the pixel struct. Register separate layers for:
- Data that doesn't swap with pixels (spatial fields)
- Downsampled grids (heat, pressure)
- Optional per-pixel data (velocity, age)

The game decides what goes in the pixel vs. separate layers based on:
- **In pixel:** Data that must swap together (material, color, damage)
- **Separate layer:** Data with different sample rate, or spatial (not pixel-bound)

### Memory Examples

| Configuration | Size | Layout |
|---------------|------|--------|
| material + color + damage + flags8 | 4 bytes | `[mat][col][dmg][flg]` |
| material + color + nibbles{dmg,var} + flags8 | 4 bytes | `[mat][col][d\|v][flg]` |
| material + flags8 | 2 bytes | `[mat][flg]` |
| material + color + flags8 + flags8 | 4 bytes | `[mat][col][flg1][flg2]` |
| material(u16) + color + flags8 | 4 bytes | `[mat][mat][col][flg]` |

Games define exactly what they need. Byte-alignment ensures shader compatibility.

### Implementation Notes

**Storage requires:** `T: Copy + Default + 'static`

**Optional traits:**
```rust
pub trait PixelCollision { fn is_solid(&self) -> bool; }
pub trait PixelDirty {
    fn is_dirty(&self) -> bool;
    fn set_dirty(&mut self, dirty: bool);
}
```

**Recommended approach:**
- Use `bitflags!` for flags (widely used, well-tested)
- Use `#[repr(C)]` for shader compatibility
- Keep pixel size small (2-8 bytes typical)

**No framework-provided bitpacking** — games bring their own tools.

### Status

**Not yet implemented.** Current framework uses hardcoded `Pixel` struct.

The modularity refactor will:
1. Add optional traits to framework (`PixelCollision`, `PixelDirty`)
2. Make storage generic over `T: Copy + Default + 'static`
3. Move `Pixel` definition to demo game
4. Upload raw bytes (shader interprets via palette LUT)

## Related Documentation

- [Pixel Format](../foundational/pixel-format.md) - Base layer specification
- [Simulation](../simulation/simulation.md) - Heat layer propagation, swap mechanics
- [Simulation Extensibility](simulation-extensibility.md) - Custom rules using layer data
- [Rendering Backends](rendering-backends.md) - Backend integration for layer rendering
- [Chunk Pooling](../chunk-management/chunk-pooling.md) - Chunk lifecycle and layer allocation
- [Chunk Persistence](../persistence/chunk-persistence.md) - Save format for persistent layers
- [Architecture Overview](../README.md)
