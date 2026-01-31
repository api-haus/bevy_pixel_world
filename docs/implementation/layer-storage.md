# Plan: Layer Storage Architecture

## Context

Current chunk storage:
- `pixels: PixelSurface` - AoS, 4-byte `Pixel` struct (material, color, damage, flags)
- `heat: Box<[u8]>` - hardcoded downsampled layer
- No layer abstraction - fields are baked into struct

Goal: Store arbitrary layer configurations per chunk, supporting:
- Different games with different layer needs
- Built-in layers (Color, Damage, Flags, Heat)
- Custom user layers (e.g., BrickLayer in demo game)
- Mixed sample rates (1:1, 4:1, etc.)

## Requirements

- **Compile-time layer sets**: Layers registered at build time, fixed per binary
- **Custom first-class**: User-defined layers equal to built-ins
- **Minimal generics**: `Chunk` stays concrete, no viral `Chunk<L>`
- **Configurable via macros**: `BrickLayer` grid size selected via macro invocation
- **Simulations as Bevy systems**: Systems take layer handles and operate on layer data

---

## Design: Registry + Typed Handles + Macros

### Layer Trait

```rust
trait Layer: 'static {
    type Element: Copy + Default + Send + Sync + 'static;

    const SAMPLE_RATE: u32;
    const NAME: &'static str;
}
```

### Base Layer (Innate)

Every chunk has a hardcoded base layer - material IDs. Not opt-in, always present:

```rust
// Hardcoded in Chunk struct, not part of LayerRegistry
struct Chunk {
    base: Box<[MaterialId]>,      // always present
    layers: Vec<LayerStorage>,    // opt-in layers via registry
    ...
}
```

### Optional Layers

```rust
struct ColorLayer;
impl Layer for ColorLayer {
    type Element = u8;
    const SAMPLE_RATE: u32 = 1;
    const NAME: &'static str = "color";
}

struct FlagsLayer;
impl Layer for FlagsLayer {
    type Element = u8;
    const SAMPLE_RATE: u32 = 1;
    const NAME: &'static str = "flags";
}

struct HeatLayer;
impl Layer for HeatLayer {
    type Element = u8;
    const SAMPLE_RATE: u32 = 4;
    const NAME: &'static str = "heat";
}
```

### BrickLayer Macro (Demo Pattern)

This macro is included in the demo game, not the engine. Users copy it into their games and adapt as needed.

Since both CHUNK_SIZE and GRID are compile-time constants, a macro generates the BrickLayer types:

```rust
/// Generates BrickLayer types with correct BrickId type and sample rates.
///
/// BrickId type:
/// - GRID² ≤ 256 → u8
/// - GRID² > 256 → u16
///
/// Creates two layers (different sample rates require separate layers):
/// - BrickIdLayer: maps each pixel to its brick (sample rate 1)
/// - BrickDamageLayer: damage per brick (sample rate = chunk_size / grid)
macro_rules! define_brick_layer {
    ($name:ident, chunk_size: $chunk:expr, grid: $grid:expr) => {
        struct $name;

        impl $name {
            const CHUNK_SIZE: u32 = $chunk;
            const GRID: u32 = $grid;
            const BRICK_COUNT: u32 = Self::GRID * Self::GRID;
            const DAMAGE_SAMPLE_RATE: u32 = Self::CHUNK_SIZE / Self::GRID;

            /// Registers both brick layers and returns handles.
            fn register(registry: &mut LayerRegistry) -> BrickLayerHandles {
                let id = registry.register::<BrickIdLayer<{ $grid * $grid }>>();
                let damage = registry.register::<BrickDamageLayer<{ $chunk / $grid }>>();
                BrickLayerHandles { id, damage }
            }
        }
    };
}

/// Convenience struct holding handles to both brick layers.
struct BrickLayerHandles {
    id: LayerHandle<BrickIdLayer>,
    damage: LayerHandle<BrickDamageLayer>,
}

// Usage at build configuration:
define_brick_layer!(GameBrickLayer, chunk_size: 512, grid: 16);
```

### LayerHandle

```rust
pub struct LayerHandle<L: Layer> {
    index: usize,
    _marker: PhantomData<L>,
}

impl Chunk {
    pub fn get<L: Layer>(&self, handle: LayerHandle<L>) -> &[L::Element] {
        let storage = &self.layers[handle.index];
        // SAFETY: handle guarantees correct type
        unsafe { storage.as_slice::<L::Element>() }
    }

    pub fn get_mut<L: Layer>(&mut self, handle: LayerHandle<L>) -> &mut [L::Element] {
        let storage = &mut self.layers[handle.index];
        unsafe { storage.as_slice_mut::<L::Element>() }
    }
}
```

### LayerStorage (Type-Erased)

```rust
struct LayerStorage {
    data: Box<[u8]>,
    element_size: usize,
    len: usize,
}

impl LayerStorage {
    unsafe fn as_slice<T>(&self) -> &[T] {
        std::slice::from_raw_parts(self.data.as_ptr() as *const T, self.len)
    }

    unsafe fn as_slice_mut<T>(&mut self) -> &mut [T] {
        std::slice::from_raw_parts_mut(self.data.as_mut_ptr() as *mut T, self.len)
    }
}
```

### Registry

```rust
#[derive(Resource)]
pub struct LayerRegistry {
    layers: Vec<LayerMeta>,
    type_to_index: HashMap<TypeId, usize>,
}

impl LayerRegistry {
    pub fn register<L: Layer>(&mut self) -> LayerHandle<L> {
        let index = self.layers.len();
        self.layers.push(LayerMeta {
            name: L::NAME,
            element_size: size_of::<L::Element>(),
            sample_rate: L::SAMPLE_RATE,
            type_id: TypeId::of::<L>(),
        });
        self.type_to_index.insert(TypeId::of::<L>(), index);
        LayerHandle { index, _marker: PhantomData }
    }
}
```

### Plugin Builder

```rust
PixelWorldPlugin::builder()
    .with_layer::<ColorLayer>()
    .with_layer::<FlagsLayer>()
    .with_layer::<HeatLayer>()
    .build()
```

### Simulations as Bevy Systems

```rust
fn falling_sand_system(
    mut chunks: Query<&mut Chunk>,
    flags: Res<LayerHandle<FlagsLayer>>,
    materials: Res<MaterialRegistry>,
) {
    for mut chunk in &mut chunks {
        let base = chunk.base();              // innate material layer
        let flag_data = chunk.get_mut(*flags); // opt-in layer via handle
        // physics logic...
    }
}

fn heat_diffusion_system(
    mut chunks: Query<&mut Chunk>,
    heat: Res<LayerHandle<HeatLayer>>,
) {
    for mut chunk in &mut chunks {
        let heat_data = chunk.get_mut(*heat);
        // diffusion logic...
    }
}

fn brick_damage_system(
    mut chunks: Query<&mut Chunk>,
    brick: Res<BrickLayerHandles>,  // contains id + damage handles
) {
    for mut chunk in &mut chunks {
        let ids = chunk.get(brick.id);
        let damage = chunk.get_mut(brick.damage);
        // accumulate damage per brick...
    }
}
```

**Scheduling**: Bevy scheduler parallelizes systems with disjoint layer access automatically.

### Chunk Allocation

```rust
impl Chunk {
    pub fn new(registry: &LayerRegistry, chunk_size: u32) -> Self {
        let layers = registry.layers.iter().map(|meta| {
            let cells_per_dim = chunk_size / meta.sample_rate;
            let cell_count = (cells_per_dim * cells_per_dim) as usize;
            let byte_count = cell_count * meta.element_size;
            LayerStorage {
                data: vec![0u8; byte_count].into_boxed_slice(),
                element_size: meta.element_size,
                len: cell_count,
            }
        }).collect();

        Chunk { layers, pos: None }
    }
}
```

---

## Advantages

| Aspect | Result |
|--------|--------|
| Type safety | ✓ Handle carries type, access is safe |
| No generics pollution | ✓ Chunk is concrete |
| Performance | ~Vec index lookup (fast) |
| BrickLayer flexibility | ✓ Macro generates correct types for any GRID |
| Bevy integration | ✓ Systems use Res<LayerHandle<T>> |

## Trade-offs

| Concern | Mitigation |
|---------|------------|
| Unsafe in storage access | Encapsulated, handle guarantees correctness |
| Layer set fixed at compile time | Macro provides flexibility within binary |

---

## Files to Create/Modify

1. **`crates/bevy_pixel_world/src/layer/mod.rs`** (new)
   - `Layer` trait
   - `LayerHandle<L>`
   - `LayerStorage`
   - `LayerRegistry`

2. **`crates/bevy_pixel_world/src/layer/builtin.rs`** (new)
   - `ColorLayer`, `DamageLayer`, `FlagsLayer`
   - `HeatLayer`, `TemperatureLayer`, `VelocityLayer`, etc.

3. **Demo game** (not in engine crate)
   - `define_brick_layer!` macro (generates two layers: BrickIdLayer + BrickDamageLayer)
   - `BrickLayerHandles` convenience struct
   - Example system using brick layers
   - Users copy this pattern into their games

4. **`crates/bevy_pixel_world/src/primitives/chunk.rs`**
   - Keep `base: Box<[MaterialId]>` (innate, always present)
   - Replace `heat: Box<[u8]>` with `layers: Vec<LayerStorage>` (opt-in)
   - Add `get`/`get_mut` methods for layer access

5. **`crates/bevy_pixel_world/src/plugin.rs`**
   - Builder API for layer registration
   - Insert `LayerHandle<T>` resources

## Verification

1. Register layers via builder
2. Create chunk, verify layer storage sizes
3. Access via handle, verify type safety
4. Define custom BrickLayer with macro, verify correct types generated
