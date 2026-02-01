# Layer Storage Architecture

## Framework vs Game Crate

The framework provides **infrastructure only**:
- `Pixel` (material + flags) as the sole innate layer
- Layer traits (`SwapLayer`, `PositionalLayer`)
- Bundle/storage macros (`define_bundle!`, `define_swap_layer!`)
- Accessor types (`LayerMut<L>`, `LayerRef<L>`)
- Iteration primitives (`PhasedIter<L>`)

The **game crate** provides everything else:
- Layer definitions (ColorLayer, DamageLayer, HeatLayer, etc.)
- Bundle compositions (e.g., FallingSandBundle for the demo game)
- **All simulations** including falling sand automata
- Material definitions and interactions

The framework ships zero simulations and zero layers beyond `Pixel`. This is the only sane design: the framework is a generic pixel-world harness; actual game mechanics belong to the game.

### Pixel Flags

The framework's `Pixel` includes 8 flag bits:
- **3 reserved** (framework-controlled): dirty, solid, falling
- **5 customizable** (game-defined): burning, wet, pixel_body, etc.

---

## Core Concept

Two-level abstraction separating semantic API from internal storage:

```
Semantic Layer (API)           →    Packed Storage (Internal)
──────────────────────────────────────────────────────────────
define_swap_layer!(ColorLayer)      ┐
define_swap_layer!(DamageLayer)     ├→  SwapUnit { pixel, color, damage }
Pixel (material + flags)            ┘

Simulation uses:                    Storage is:
  LayerMut<ColorLayer>                Surface<SwapUnit>
  color.set(pos, 42)                  chunk.data[idx].color = 42
```

Simulations use layer types (preserved semantics). Framework generates packed storage (optimized memory layout). Swap is single atomic operation.

---

## Layer Traits

### SwapLayer (Moves with Pixel)

Data belongs to the pixel. When `Pixel` swaps, all swap-layers swap atomically as one unit.

```rust
/// Swap-follow layer - data moves with pixel
trait SwapLayer: 'static {
    type Element: Copy + Default;
    const NAME: &'static str;
}

// Definition macro
macro_rules! define_swap_layer {
    ($name:ident, $elem:ty, $label:literal) => {
        struct $name;
        impl SwapLayer for $name {
            type Element = $elem;
            const NAME: &'static str = $label;
        }
    };
}

// Game crate defines its layers:
define_swap_layer!(ColorLayer, u8, "color");
define_swap_layer!(DamageLayer, u8, "damage");
define_swap_layer!(GroupingLayer, u16, "grouping");
```

### PositionalLayer (Stays at Location)

Data belongs to the location. Pixels pass through without affecting it. Stored as separate SoA arrays.

```rust
/// Positional layer - data stays at location
trait PositionalLayer: 'static {
    type Element: Copy + Default;
    const NAME: &'static str;
    const SAMPLE_RATE: u32;  // 1 = per-pixel, 4 = 4×4 regions
}

// Definition macro
macro_rules! define_positional_layer {
    ($name:ident, $elem:ty, $label:literal, $rate:expr) => {
        struct $name;
        impl PositionalLayer for $name {
            type Element = $elem;
            const NAME: &'static str = $label;
            const SAMPLE_RATE: u32 = $rate;
        }
    };
}

// Game crate defines its positional layers:
define_positional_layer!(HeatLayer, u8, "heat", 4);       // 4×4 downsample
define_positional_layer!(PressureLayer, u16, "pressure", 8);  // 8×8 downsample
```

---

## Bundle Composition

The `define_bundle!` macro composes swap-layers into packed internal structs.

```rust
// Framework provides ONLY the base Pixel type (not a bundle macro invocation)
// Games build bundles on top of Pixel:

// Game crate (e.g., falling-sand-demo) defines its bundles:
define_bundle! {
    /// Falling sand game bundle (4 bytes)
    FallingSandBundle = Pixel + ColorLayer + DamageLayer;

    /// Builder variant with grouping (8 bytes, padded)
    BuilderBundle = Pixel + ColorLayer + DamageLayer + GroupingLayer;
}
```

### Generated Code (FallingSandBundle)

The macro generates:

```rust
/// Internal packed struct - not exposed to simulations
#[repr(C, align(4))]
#[derive(Clone, Copy, Default)]
struct FallingSandBundleSwapUnit {
    material: MaterialId,  // 1 byte (from Pixel)
    flags: PixelFlags,     // 1 byte (from Pixel)
    color: u8,             // 1 byte (from ColorLayer)
    damage: u8,            // 1 byte (from DamageLayer)
}

/// Layer accessor trait - maps layer type to field
trait LayerAccess<L: SwapLayer> {
    fn get(&self) -> L::Element;
    fn set(&mut self, v: L::Element);
}

/// Implementation for ColorLayer
impl LayerAccess<ColorLayer> for FallingSandBundleSwapUnit {
    fn get(&self) -> u8 { self.color }
    fn set(&mut self, v: u8) { self.color = v }
}

/// Implementation for DamageLayer
impl LayerAccess<DamageLayer> for FallingSandBundleSwapUnit {
    fn get(&self) -> u8 { self.damage }
    fn set(&mut self, v: u8) { self.damage = v }
}

/// Implementation for Pixel
impl LayerAccess<Pixel> for FallingSandBundleSwapUnit {
    fn get(&self) -> PixelData { PixelData { material: self.material, flags: self.flags } }
    fn set(&mut self, v: PixelData) {
        self.material = v.material;
        self.flags = v.flags;
    }
}
```

---

## Chunk Structure

```rust
pub struct Chunk<S: SwapUnit> {
    /// Packed swap-layer data (AoS - Array of Structures)
    pub swap_data: Surface<S>,

    /// Positional layers (SoA - Structure of Arrays, separate arrays)
    positional: PositionalLayers,

    /// Metadata
    tile_dirty_rects: Box<[TileDirtyRect]>,
    tile_collision_dirty: Box<[bool]>,
    pos: Option<ChunkPos>,
}

/// Registry of positional layer arrays
struct PositionalLayers {
    heat: Option<Box<[u8]>>,       // if HeatLayer registered
    pressure: Option<Box<[u16]>>,  // if PressureLayer registered
    // ... additional positional layers
}
```

### Memory Layout

```
FallingSandBundle Chunk (512×512):
┌────────────────────────────────────────────────────────┐
│ SwapUnit[0..262144] (AoS, 4 bytes each)                │  ← 1 MB
│ ┌──────────┬───────┬───────┬────────┐                  │
│ │ material │ flags │ color │ damage │ × 262144         │
│ └──────────┴───────┴───────┴────────┘                  │
├────────────────────────────────────────────────────────┤
│ Heat[0..16384] (SoA, 4×4 downsample, 1 byte each)      │  ← 16 KB
│ Pressure[0..4096] (SoA, 8×8 downsample, 2 bytes each)  │  ← 8 KB
└────────────────────────────────────────────────────────┘
```

---

## Simulation API

Simulations use layer types. Framework maps to internal storage via `LayerMut<L>` / `LayerRef<L>`.

```rust
// In game crate - framework provides no simulations
fn falling_sand_sim(
    iter: PhasedIter<Pixel>,
    mut pixels: LayerMut<Pixel>,       // Access via layer type
    mut color: LayerMut<ColorLayer>,   // Access via layer type
    materials: Res<MaterialRegistry>,
) {
    iter.for_each(|frag| {
        let pos = frag.pos();
        let pixel = pixels.get(pos);
        if pixel.is_void() { return; }

        if let Some(target) = try_fall(pos, &pixels, &materials) {
            pixels.swap(pos, target);
            // ColorLayer, DamageLayer swap automatically (same SwapUnit)
        }
    });
}
```

### LayerMut Implementation

```rust
/// Provides layer-typed access to packed storage
pub struct LayerMut<'w, L: SwapLayer> {
    canvas: &'w mut Canvas,
    _marker: PhantomData<L>,
}

impl<L: SwapLayer> LayerMut<'_, L> {
    pub fn get(&self, pos: WorldPos) -> L::Element {
        let chunk = self.canvas.get(pos.chunk())?;
        let unit = &chunk.swap_data[pos.local_index()];
        <S as LayerAccess<L>>::get(unit)  // Compiler resolves via trait
    }

    pub fn set(&mut self, pos: WorldPos, value: L::Element) {
        let chunk = self.canvas.get_mut(pos.chunk())?;
        let unit = &mut chunk.swap_data[pos.local_index()];
        <S as LayerAccess<L>>::set(unit, value)
    }
}
```

### LayerRef (Read-Only)

```rust
/// Read-only layer access
pub struct LayerRef<'w, L: SwapLayer> {
    canvas: &'w Canvas,
    _marker: PhantomData<L>,
}

impl<L: SwapLayer> LayerRef<'_, L> {
    pub fn get(&self, pos: WorldPos) -> L::Element {
        let chunk = self.canvas.get(pos.chunk())?;
        let unit = &chunk.swap_data[pos.local_index()];
        <S as LayerAccess<L>>::get(unit)
    }
}
```

---

## Swap Mechanics

Since all swap-layers are packed into one struct, swap is a single atomic operation:

```rust
impl Canvas {
    pub fn swap(&mut self, a: WorldPos, b: WorldPos) {
        // Single memcpy for entire SwapUnit
        if a.chunk() == b.chunk() {
            let chunk = self.get_mut(a.chunk());
            chunk.swap_data.swap(a.local_index(), b.local_index());
        } else {
            let (ca, cb) = self.get_two_mut(a.chunk(), b.chunk());
            std::mem::swap(
                &mut ca.swap_data[a.local_index()],
                &mut cb.swap_data[b.local_index()],
            );
        }
        // All swap-layers (color, damage, etc.) swapped atomically
    }
}
```

**Key properties:**

| Property | Implementation |
|----------|----------------|
| Atomic | Single memory operation swaps all layer data |
| No loop | No iteration over layer indices |
| Cache-friendly | Contiguous memory access |
| Cross-chunk safe | Canvas handles both chunks |

---

## Positional Layer Access

Positional layers use separate accessor types since they're stored in SoA:

```rust
pub struct PositionalMut<'w, L: PositionalLayer> {
    canvas: &'w mut Canvas,
    _marker: PhantomData<L>,
}

impl<L: PositionalLayer> PositionalMut<'_, L> {
    pub fn get(&self, pos: WorldPos) -> L::Element {
        let chunk = self.canvas.get(pos.chunk())?;
        let cell_pos = pos.local() / L::SAMPLE_RATE;
        chunk.positional.get::<L>(cell_pos)
    }

    pub fn set(&mut self, pos: WorldPos, value: L::Element) {
        let chunk = self.canvas.get_mut(pos.chunk())?;
        let cell_pos = pos.local() / L::SAMPLE_RATE;
        chunk.positional.set::<L>(cell_pos, value)
    }
}
```

---

## Bundle Memory Calculations

| Bundle | Layers | Raw Size | Aligned | Per Chunk (512²) |
|--------|--------|----------|---------|------------------|
| Pixel only (framework) | Pixel | 2B | 2B | 512 KB |
| FallingSandBundle (game) | +Color+Damage | 4B | 4B | 1 MB |
| BuilderBundle (game) | +Grouping | 6B | 8B | 2 MB |

---

## Plugin Builder API

```rust
// In game crate - framework provides no simulations
PixelWorldPlugin::builder()
    .with_bundle(FallingSandBundle)          // Game's bundle (Pixel + Color + Damage)
    .with_bundle_upload(UploadSchedule::Dirty)

    .with_positional::<HeatLayer>()
        .upload(UploadSchedule::DirtyThrottled(4))  // Every 4th tick if changed

    .with_positional::<VelocityLayer>()      // No .upload() = CPU-only

    .with_simulations((
        falling_sand_sim,
        heat_diffusion_sim,
    ))
    .build()
```

---

## GPU Upload Scheduling

Layers declare when their data uploads to GPU. The framework manages dirty tracking and upload timing.

### UploadSchedule Enum

All schedules require the layer to be dirty. Unchanged data never uploads.

```rust
enum UploadSchedule {
    /// Upload every tick if layer dirty
    Dirty,

    /// Upload every N ticks if layer dirty
    DirtyThrottled(u32),
}

// No .upload() call = layer is CPU-only, never uploaded to GPU
```

### Upload Execution

All schedules implicitly check layer-specific dirty status. Unchanged layers never upload.

```rust
// Framework runs after simulation tick completes
fn upload_system(
    chunks: Query<&Chunk>,
    bundle_schedule: Res<BundleUploadSchedule>,
    positional_schedules: Res<PositionalUploadSchedules>,
    tick: Res<SimTick>,
    mut gpu: ResMut<GpuContext>,
) {
    for chunk in chunks.iter() {
        // Bundle (SwapUnit) upload - only if bundle data changed
        if bundle_schedule.should_upload(&chunk.swap_dirty, tick.0) {
            gpu.upload_bundle_texture(chunk);
            chunk.swap_dirty = false;
        }

        // Positional layer uploads - only if that layer changed
        for (layer_id, schedule) in positional_schedules.iter() {
            if schedule.should_upload(&chunk.positional_dirty[layer_id], tick.0) {
                gpu.upload_positional_texture(chunk, layer_id);
                chunk.positional_dirty[layer_id] = false;
            }
        }
    }
}

impl UploadSchedule {
    fn should_upload(&self, dirty: bool, tick: u64) -> bool {
        if !dirty { return false; }

        match self {
            Self::Dirty => true,
            Self::DirtyThrottled(n) => tick % (*n as u64) == 0,
        }
    }
}

// Layers without .upload() are not in positional_schedules — never uploaded
```
```

### Typical Configuration

| Layer | Schedule | Rationale |
|-------|----------|-----------|
| SwapUnit bundle | `Dirty` | Immediate visual feedback |
| HeatLayer | `DirtyThrottled(4)` | Slow diffusion; shader interpolates |
| GlowLayer | `DirtyThrottled(2)` | Visual effect; slight delay acceptable |
| VelocityLayer | (none) | Simulation-only; not rendered |
| PressureLayer | (none) | Affects behavior, not visual |

---

## Key Design Decisions

1. **Simulations use layer types** - preserves semantics, enables Bevy's dependency inference
2. **Storage is opaque packed struct** - single swap operation, cache-friendly
3. **Macro generates accessors** - type-safe bridge between semantic and storage
4. **Positional layers stay SoA** - different access pattern, often downsampled
5. **No runtime layer registry** - bundles fixed at compile time for maximum optimization
6. **Framework is barebones** - only Pixel layer + infrastructure; games define everything else

---

## Files to Create/Modify

### Framework Crate

1. **`src/layer/mod.rs`**
   - `SwapLayer`, `PositionalLayer` traits
   - `define_swap_layer!`, `define_positional_layer!` macros

2. **`src/layer/bundle.rs`**
   - `define_bundle!` macro
   - `SwapUnit` trait for generated structs
   - `LayerAccess<L>` trait

3. **`src/layer/access.rs`**
   - `LayerMut<L>`, `LayerRef<L>` for swap layers
   - `PositionalMut<L>`, `PositionalRef<L>` for positional layers

4. **`src/primitives/pixel.rs`**
   - `Pixel` struct (material + flags)
   - `PixelFlags` with 3 reserved + 5 customizable bits

5. **`src/primitives/chunk.rs`**
   - `Chunk<S: SwapUnit>` with generic swap unit
   - `PositionalLayers` storage

### Game Crate (e.g., falling-sand-demo)

1. **`src/layers.rs`**
   - ColorLayer, DamageLayer, GroupingLayer definitions
   - HeatLayer, PressureLayer definitions

2. **`src/bundle.rs`**
   - FallingSandBundle, BuilderBundle definitions

3. **`src/simulation/mod.rs`**
   - falling_sand_sim, heat_diffusion_sim, etc.
