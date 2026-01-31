# Plan: Layer Storage Architecture

## Context

**Note:** This section describes the *old* architecture being replaced.

Current chunk storage (old):
- `pixels: PixelSurface` - AoS, 4-byte `Pixel` struct (material, color, damage, flags)
- `heat: Box<[u8]>` - hardcoded downsampled layer
- No layer abstraction - fields are baked into struct

Goal: Store arbitrary layer configurations per chunk, supporting:
- Different games with different layer needs
- Built-in PixelLayer (Material + Flags, 2 bytes)
- Built-in layers (Color, Damage)
- Custom user layers (e.g., GroupingLayer for building games)
- Mixed sample rates (1:1, 4:1, etc.)

## Requirements

- **Compile-time layer sets**: Layers registered at build time, fixed per binary
- **Custom first-class**: User-defined layers equal to built-ins
- **Minimal generics**: `Chunk` stays concrete, no viral `Chunk<L>`
- **World-coordinate access**: Simulations use `WorldPos`, not chunk-local indices
- **Iterator-driven scheduling**: Schedule mode determined by iterator type in system signature
- **Layer categories**: Swap-follow (moves with pixel) vs positional (stays at location)

---

## Architecture Overview

Three-layer design separating storage, access, and iteration:

```
┌─────────────────────────────────────────────────────────────┐
│  Simulation System                                          │
│  fn my_sim(iter: PhasedIter<L>, mut pixels: ResMut<L>)      │
└─────────────────────────────────────────────────────────────┘
          │                           │
          │ yields WorldFragment      │ PixelAccess trait
          ▼                           ▼
┌─────────────────────┐    ┌─────────────────────────────────┐
│  PhasedIter<L>      │    │  LayerResource<L>               │
│  (SystemParam)      │    │  wraps Canvas + layer index     │
│  4-phase iteration  │    │  get/set/swap via WorldPos      │
│  closure-based API  │    │  swap triggers swap-follow      │
└─────────────────────┘    └─────────────────────────────────┘
                                      │
                                      │ routes to correct chunk
                                      ▼
                           ┌─────────────────────────────────┐
                           │  Canvas                         │
                           │  HashMap<ChunkPos, &mut Chunk>  │
                           └─────────────────────────────────┘
                                      │
                                      │ layer index lookup
                                      ▼
                           ┌─────────────────────────────────┐
                           │  Chunk                          │
                           │  pixel: Box<[Pixel]>            │
                           │  layers: Vec<LayerStorage>      │
                           └─────────────────────────────────┘
                                      │
                                      │ type-erased bytes
                                      ▼
                           ┌─────────────────────────────────┐
                           │  LayerStorage                   │
                           │  data: Box<[u8]>                │
                           │  category: LayerCategory        │
                           └─────────────────────────────────┘
```

## Layer Categories

Layers fall into two categories based on whether their data belongs to the **pixel** or the **location**:

### Category Decision Tree

```
Does the data belong to the pixel or the location?
├── Pixel (moves with pixel) → Swap-follow
│   - ColorLayer: pixel's palette index
│   - DamageLayer: accumulated damage on pixel
│   - GroupingLayer: pixel's group membership
│   - TemperatureLayer: pixel IS hot
│
└── Location (pixel passes through) → Positional
    - HeatLayer: ambient heat at location
    - PressureLayer: fluid pressure gradient
    - WindLayer: environmental wind
    - RadiationLayer: radiation at position
```

### Swap-Follow Behavior

When PixelLayer swaps, registered swap-follow layers swap automatically:

```rust
// ✓ Recommended: swap on PixelLayer, others follow
pixels.swap(a, b);  // ColorLayer, DamageLayer swap automatically

// ⚠ Allowed: direct swap on any layer (at your own risk)
color.swap(a, b);   // works, but you're responsible for consistency
```

All layers support `swap()`—use at your own risk for maintaining consistency.

---

## Layer 1: Storage (Per-Chunk)

### Layer Trait

Declares element type, sample rate, and name:

```rust
trait Layer: 'static {
    type Element: Copy + Default + Send + Sync + 'static;
    const SAMPLE_RATE: u32;
    const NAME: &'static str;
}
```

### LayerStorage (Type-Erased)

Each chunk holds type-erased layer data:

```rust
struct LayerStorage {
    data: Box<[u8]>,
    element_size: usize,
    len: usize,
}

impl LayerStorage {
    /// SAFETY: Caller must ensure T matches the registered element type
    unsafe fn as_slice<T>(&self) -> &[T] {
        std::slice::from_raw_parts(self.data.as_ptr() as *const T, self.len)
    }

    unsafe fn as_slice_mut<T>(&mut self) -> &mut [T] {
        std::slice::from_raw_parts_mut(self.data.as_mut_ptr() as *mut T, self.len)
    }
}
```

### Chunk Structure

PixelLayer (material + flags) is always present. Optional layers stored in registry order:

```rust
struct Chunk {
    pixel: Box<[Pixel]>,          // innate, always present (2 bytes each)
    layers: Vec<LayerStorage>,    // opt-in, indexed by LayerHandle
    // ... dirty rects, collision flags, etc.
}

#[repr(C)]
struct Pixel {
    material: MaterialId,  // u8
    flags: PixelFlags,     // u8
}
```

### LayerRegistry

Tracks registered layers, provides metadata for chunk allocation:

```rust
#[derive(Resource)]
pub struct LayerRegistry {
    layers: Vec<LayerMeta>,
    type_to_index: HashMap<TypeId, usize>,
    swap_follow_layer_indices: Vec<usize>,  // populated during registration
}

struct LayerMeta {
    name: &'static str,
    element_size: usize,
    sample_rate: u32,
    type_id: TypeId,
    category: LayerCategory,
}

enum LayerCategory {
    SwapFollow,  // Moves with pixel; auto-swaps when PixelLayer swaps
    Positional,  // Stays at location
}

impl LayerRegistry {
    fn register<L: Layer>(&mut self, category: LayerCategory) -> usize {
        let idx = self.layers.len();
        self.layers.push(LayerMeta { /* ... */ category });
        self.type_to_index.insert(TypeId::of::<L>(), idx);

        // Track swap-follow layers for PixelLayer to use
        if matches!(category, LayerCategory::SwapFollow) {
            self.swap_follow_layer_indices.push(idx);
        }
        idx
    }
}
```

When `LayerResource<PixelLayer>` is created, it clones `swap_follow_layer_indices` from the registry.

### Chunk Allocation

Chunks allocate storage for all registered layers:

```rust
impl Chunk {
    pub fn new(registry: &LayerRegistry, chunk_size: u32) -> Self {
        let pixel_count = (chunk_size * chunk_size) as usize;

        let layers = registry.layers.iter().map(|meta| {
            let cells_per_dim = chunk_size / meta.sample_rate;
            let cell_count = (cells_per_dim * cells_per_dim) as usize;
            LayerStorage::new(cell_count, meta.element_size, meta.category)
        }).collect();

        Chunk {
            pixel: vec![Pixel::default(); pixel_count].into(),
            layers,
            ..Default::default()
        }
    }
}
```

---

## Layer 2: World-Level Access

### PixelAccess Trait

Framework provides uniform access API for all layers:

```rust
trait PixelAccess {
    type Element;

    fn get(&self, pos: WorldPos) -> Self::Element;
    fn set(&mut self, pos: WorldPos, value: Self::Element);
    fn swap(&mut self, a: WorldPos, b: WorldPos);
    fn swap_unchecked(&mut self, a: WorldPos, b: WorldPos);
}
```

All layers support `swap()`. PixelLayer swap triggers automatic swap on registered swap-follow layers. Direct swap on other layers is allowed—use at your own risk.

### LayerResource

Each registered layer becomes a world-level resource wrapping Canvas:

```rust
/// World-level access to a specific layer across all chunks
#[derive(Resource)]
struct LayerResource<L: Layer> {
    canvas: Canvas,           // shared reference to chunk map
    layer_index: usize,       // index into Chunk.layers
    _marker: PhantomData<L>,
}

impl<L: Layer> PixelAccess for LayerResource<L> {
    type Element = L::Element;

    fn get(&self, pos: WorldPos) -> L::Element {
        let (chunk_pos, local) = pos.to_chunk_and_local();
        let chunk = self.canvas.get(chunk_pos)?;
        let storage = &chunk.layers[self.layer_index];
        unsafe { storage.as_slice::<L::Element>()[local_index] }
    }

    fn swap(&mut self, a: WorldPos, b: WorldPos) {
        // Swap just this layer's data
        self.canvas.swap_layer(self.layer_index, a, b);
    }
}
```

### PixelLayer Swap-Follow Implementation

`LayerResource<PixelLayer>` is special—it holds the swap-follow indices and swaps all layers atomically:

```rust
#[derive(Resource)]
struct LayerResource<PixelLayer> {
    canvas: Canvas,
    layer_index: usize,  // always 0 for PixelLayer (innate)
    swap_follow_layer_indices: Vec<usize>,  // indices of swap-follow layers
}

impl PixelAccess for LayerResource<PixelLayer> {
    fn swap(&mut self, a: WorldPos, b: WorldPos) {
        // 1. Swap PixelLayer
        self.canvas.swap_layer(0, a, b);

        // 2. Swap all swap-follow layers inline
        for &layer_idx in &self.swap_follow_layer_indices {
            self.canvas.swap_layer(layer_idx, a, b);
        }
    }
}
```

### Canvas Swap Implementation

Canvas handles the actual byte swapping, including cross-chunk:

```rust
impl Canvas {
    fn swap_layer(&mut self, layer_idx: usize, a: WorldPos, b: WorldPos) {
        let (chunk_a, local_a) = a.to_chunk_and_local();
        let (chunk_b, local_b) = b.to_chunk_and_local();

        if chunk_a == chunk_b {
            // Same chunk: direct swap
            let chunk = self.get_mut(chunk_a);
            chunk.layers[layer_idx].swap(local_a, local_b);
        } else {
            // Cross-chunk: get both, swap elements
            let (ca, cb) = self.get_two_mut(chunk_a, chunk_b);
            let val_a = ca.layers[layer_idx].get(local_a);
            let val_b = cb.layers[layer_idx].get(local_b);
            ca.layers[layer_idx].set(local_a, val_b);
            cb.layers[layer_idx].set(local_b, val_a);
        }
    }
}
```

**Key properties:**

| Property | Implementation |
|----------|----------------|
| Atomic | All swap-follow layers swap in same function call |
| No backtracking | Single pass through swap-follow indices |
| No streaming | Direct memory operations |
| Cross-chunk safe | Canvas handles both chunks in one call |

### Type Aliases for Ergonomics

Simulations use `ResMut<L>` which resolves to `ResMut<LayerResource<L>>`:

```rust
// In system signatures, these are equivalent:
fn my_sim(mut material: ResMut<PixelLayer>) { ... }
fn my_sim(mut material: ResMut<LayerResource<PixelLayer>>) { ... }
```

The framework registers `LayerResource<L>` as the resource for each layer type.

---

## Layer 3: Iteration (Schedule Mode)

### Iterator Types Determine Schedule Mode

The iterator type in a simulation's signature selects the scheduling strategy:

| Iterator | Schedule Mode | Safety |
|----------|---------------|--------|
| `PhasedIter<L>` | 4-phase checkerboard | Safe for local ops |
| `ParallelIter<L>` | All pixels at once | **Unsafe** (consumer handles races) |
| (regular loop) | Sequential | Always safe |

### PhasedIter

Wraps existing `parallel_simulate` infrastructure. System runs 4 times per tick:

```rust
/// Yields WorldPos in checkerboard phase order
struct PhasedIter<'w, L: Layer> {
    phase: Phase,              // current phase (A, B, C, D)
    tiles: &'w [TilePos],      // tiles for this phase
    current_tile: usize,
    current_pixel: usize,
    _marker: PhantomData<L>,
}

impl<L: Layer> SystemParam for PhasedIter<'_, L> {
    // Framework calls system 4 times, advancing phase each time
    // Barrier between phases ensures spatial isolation
}
```

### ParallelIter

All pixels yielded simultaneously. No synchronization:

```rust
/// Yields all WorldPos in parallel (unsafe)
struct ParallelIter<'w, L: Layer> {
    canvas: &'w Canvas,
    _marker: PhantomData<L>,
}
```

### Sequential (No Special Iterator)

Just use `layer.iter_all()`:

```rust
fn complex_sim(mut material: ResMut<PixelLayer>) {
    for pos in material.iter_all() {
        // Single-threaded, can mutate global state
    }
}
```

---

## Built-in Layers

### PixelLayer (Innate)

```rust
struct PixelLayer;  // Always present (innate)
impl Layer for PixelLayer {
    type Element = Pixel;  // { material: MaterialId, flags: PixelFlags }
    const SAMPLE_RATE: u32 = 1;
    const NAME: &'static str = "pixel";
}

#[repr(C)]
struct Pixel {
    material: MaterialId,  // u8
    flags: PixelFlags,     // u8
}
```

### Core Layers (Default Bundle)

```rust
struct ColorLayer;  // Swap-follow
impl Layer for ColorLayer {
    type Element = ColorIndex;
    const SAMPLE_RATE: u32 = 1;
    const NAME: &'static str = "color";
}

struct DamageLayer;  // Swap-follow
impl Layer for DamageLayer {
    type Element = u8;
    const SAMPLE_RATE: u32 = 1;
    const NAME: &'static str = "damage";
}
```

### GroupingLayer (Builder Bundle)

```rust
struct GroupingLayer;  // Swap-follow
impl Layer for GroupingLayer {
    type Element = GroupingId;  // u16: 0 = none, 1+ = group
    const SAMPLE_RATE: u32 = 1;
    const NAME: &'static str = "grouping";
}
```

### Downsampled Layers (Opt-in, Positional)

```rust
struct HeatLayer;  // Positional (NOT swap-follow)
impl Layer for HeatLayer {
    type Element = u8;
    const SAMPLE_RATE: u32 = 4;  // 4×4 pixels per cell
    const NAME: &'static str = "heat";
}
```

For downsampled layers, divide coordinates explicitly:

```rust
fn heat_sim(iter: PhasedIter<HeatLayer>, mut heat: ResMut<HeatLayer>) {
    iter.for_each(|frag| {
        // pos is already in heat-cell coordinates
        let value = heat.get(frag.pos());
    });
}
```

---

## Plugin Builder API

### Registration

```rust
PixelWorldPlugin::builder()
    .with_bundle(DefaultBundle)     // PixelLayer + Color + Damage
    .with_layer::<GroupingLayer>().swap_follow()  // explicit swap-follow
    .with_layer::<HeatLayer>()      // positional (default)
    .with_layer::<PressureLayer>()
    .with_simulations((
        falling_sand_sim,           // PhasedIter<PixelLayer>
        heat_diffusion_sim,         // PhasedIter<HeatLayer>
    ))
    .with_simulations((
        decay_sim,
        interaction_sim,
    ).chain())                      // Force sequential
    .build()
```

### Execution Model

- Tuples run in parallel (if layer deps allow)
- `.chain()` forces sequential ordering
- Bevy scheduler handles inter-system parallelism based on declared layer access

---

## Example Simulations

### Falling Sand (PhasedParallel)

```rust
fn falling_sand_sim(
    iter: PhasedIter<PixelLayer>,
    mut pixels: ResMut<PixelLayer>,
    materials: Res<MaterialRegistry>,
) {
    iter.for_each(|frag| {
        if let Some(target) = try_fall_and_slide(frag.pos(), &pixels, &materials) {
            pixels.swap(frag.pos(), target);
            // ColorLayer, DamageLayer, GroupingLayer swap automatically
        }
    });
}
```

### Heat Diffusion (PhasedParallel, Downsampled)

```rust
fn heat_diffusion_sim(
    iter: PhasedIter<HeatLayer>,
    mut heat: ResMut<HeatLayer>,
) {
    iter.for_each(|frag| {
        let neighbors_avg = heat.neighbors_avg(frag.pos());
        let current = heat.get(frag.pos());
        heat.set(frag.pos(), (current + neighbors_avg) / 2);
    });
}
```

### Complex Interaction (Sequential)

```rust
fn complex_interaction_sim(
    mut pixels: ResMut<PixelLayer>,
    mut global_state: ResMut<InteractionState>,
) {
    for pos in pixels.iter_all() {
        // No special iterator = sequential
        global_state.process(pos, &mut pixels);
    }
}
```

---

## GroupingLayer Usage

GroupingLayer enables unified brick/pixel-body mechanics. See [Grouping](../arhitecture/modularity/grouping.md) for the full model.

```rust
// Register with swap-follow
PixelWorldPlugin::builder()
    .with_bundle(DefaultBundle)
    .with_layer::<GroupingLayer>().swap_follow()
    .build()

// In simulation: group membership moves with pixel
fn falling_sand_sim(
    iter: PhasedIter<PixelLayer>,
    mut pixels: ResMut<PixelLayer>,
    groups: Res<GroupRegistry>,
) {
    iter.for_each(|frag| {
        if let Some(target) = try_fall(frag.pos(), &pixels) {
            pixels.swap(frag.pos(), target);
            // GroupingLayer swaps automatically
        }
    });
}
```

---

## Advantages

| Aspect | Result |
|--------|--------|
| Type safety | ✓ Layer type in signature guarantees correct element type |
| No generics pollution | ✓ Chunk is concrete |
| World-coordinate access | ✓ Systems work with WorldPos, not chunk internals |
| Schedule mode selection | ✓ Iterator type determines parallelism |
| Bevy integration | ✓ Layer deps → automatic inter-system parallelism |

## Trade-offs

| Concern | Mitigation |
|---------|------------|
| Unsafe in storage access | Encapsulated in framework, type-checked at resource level |
| Layer set fixed at compile time | Macro provides flexibility within binary |
| Canvas indirection | Tile-local access eliminates per-pixel lookup (see below) |

---

## Potential Future Optimizations

These are hypotheses for future investigation. **Benchmark before implementing.**

### Context: Dirty Rect Tracking

Dirty rect tracking already eliminates ~90% of tiles from simulation under typical workloads. Only actively changing regions are iterated.

This means:
- Per-pixel overhead only applies to ~10% of world
- The biggest optimization is already in place (skipping dormant tiles)
- Micro-optimizations have diminishing returns

### Hypothesis: Cached Chunk Neighborhood

Current WorldPos API does HashMap lookup per pixel access. Potential optimization: pre-resolve chunk neighborhood per tile.

**Idea:** For any tile, we know which chunks it can access (center + up to 8 neighbors). Cache these before iterating:

```rust
// Hypothetical API
iter.for_each_tile(|tile: TileContext| {
    for pos in tile.iter_positions() {
        let pixel = tile.get(pos);     // O(1) via cached refs?
        tile.swap(pos, pos.below());   // cross-chunk handled uniformly
    }
});
```

**Questions to benchmark:**
- Is HashMap lookup actually the bottleneck? (May be well-optimized)
- Does neighborhood caching beat branch predictor on hot HashMap?
- What's the setup overhead for edge tiles with partial neighborhoods?

### Hypothesis: Swap-Follow Loop Unrolling

Current design loops through swap-follow indices. For common bundles (2-3 layers), could unroll.

**Questions:**
- Is the loop measurable overhead at all?
- Does `SmallVec<[_; 4]>` already optimize this sufficiently?

### Known: Memory Layout

Layers use SoA. Tile iteration accesses mostly-contiguous regions:

```
Per-tile access (32×32 = 1024 pixels):
  PixelLayer: 2 KB
  ColorLayer: 1 KB
  → Fits in L1 cache
```

This is likely already cache-friendly. Row-major layout benefits GPU upload.

---

## Files to Create/Modify

1. **`src/layer/mod.rs`** (new)
   - `Layer` trait
   - `LayerStorage`
   - `LayerRegistry`, `LayerMeta`

2. **`src/layer/access.rs`** (new)
   - `PixelAccess` trait
   - `LayerResource<L>` implementing PixelAccess

3. **`src/layer/iter.rs`** (new)
   - `PhasedIter<L>` wrapping existing parallel_simulate
   - `ParallelIter<L>` for unsafe all-at-once

4. **`src/layer/builtin.rs`** (new)
   - `PixelLayer` (innate: Material + Flags), `ColorLayer`, `DamageLayer`
   - `HeatLayer`, etc.

5. **`src/primitives/chunk.rs`**
   - Add `base: Box<[MaterialId]>` (innate)
   - Replace `heat` with `layers: Vec<LayerStorage>`

6. **`src/lib.rs`**
   - Builder API: `.with_layer::<L>()`, `.with_simulations()`
   - Bundle presets: `MinimalBundle`, `DefaultBundle`

7. **`src/scheduling/blitter.rs`**
   - Extract iteration logic for PhasedIter to wrap

## Resolved Questions

1. **Swap-follow coordination**: Framework-tracked via `.swap_follow()` at registration. PixelLayer swap triggers automatic swap on all registered swap-follow layers.

2. **Canvas lifetime**: SystemParam with explicit lifetime, rebuilt each tick (current approach).

3. **Downsampled iteration**: Heat cells (matches storage granularity). Iterators yield positions in the layer's native resolution.

4. **Cross-chunk swap-follow**: Guaranteed by system ordering. All pixel simulation systems are orchestrated by the framework with declarative syntax, scheduled at the same fixed tickrate. No locking required—cross-chunk pixel access is safe by construction.

## Verification

1. Register layers via builder, verify resources created
2. Create chunk, verify layer storage sizes match sample rates
3. PhasedIter yields correct positions per phase
4. Cross-chunk swap via PixelAccess routes correctly
5. Bevy parallelizes systems with disjoint layer writes
