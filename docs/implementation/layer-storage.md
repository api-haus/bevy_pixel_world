# Layer Storage Architecture

> **Status: Planned Architecture**
>
> This document describes the planned storage model. Current implementation uses a monolithic `Pixel` struct in the framework.

## Philosophy: Radical Modularity

**Framework provides:**
- `PixelData` trait — minimal interface for collision and scheduling
- Generic storage: `Surface<T>`, `Chunk<T>`, `Canvas<T>`, `PixelWorld<T>`
- Constraint: `T: PixelData` (is_solid, is_dirty, set_dirty)
- Collision mesh generation (uses `is_solid`)
- Dirty tracking (uses `is_dirty`, `set_dirty`)
- Iteration primitives (checkerboard phasing)
- Rendering infrastructure (game provides color extraction)
- Optional: Separate layer storage for SoA data

**Framework does NOT provide:**
- Any pixel type definition
- Material system
- Bitpacking macros (use `bitflags!` or manual)
- Simulations

**Game crate provides everything else:**
- Pixel struct implementing `PixelData`
- Material system
- All simulations
- All game-specific behavior

The framework is a generic spatial data structure library. The demo game is the reference implementation.

---

## Two Storage Patterns

### 1. Pixel Struct (AoS)

Game-defined struct stored in contiguous array. All fields swap atomically.

```rust
// Game crate defines this (using bitflags! or manual bit ops)
use bitflags::bitflags;

bitflags! {
    #[derive(Clone, Copy, Default)]
    pub struct PixelFlags: u8 {
        const DIRTY = 0x01;
        const SOLID = 0x02;
        // ... more flags
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

// Implement the minimal trait
impl PixelData for GamePixel {
    fn is_solid(&self) -> bool { self.flags.contains(PixelFlags::SOLID) }
    fn is_dirty(&self) -> bool { self.flags.contains(PixelFlags::DIRTY) }
    fn set_dirty(&mut self, v: bool) { self.flags.set(PixelFlags::DIRTY, v); }
}

// Framework stores it generically
pub struct Chunk<T: PixelData> {
    pixels: Surface<T>,  // Array of GamePixel
    // ...
}
```

### 2. Separate Layers (SoA)

Optional additional data stored in separate arrays. Used for:
- Data that doesn't swap with pixels (spatial fields)
- Downsampled grids (heat, pressure)
- Optional per-pixel data (velocity, age)

```rust
// Game crate defines these (API TBD)
struct HeatLayer;
impl Layer for HeatLayer {
    type Element = u8;
    const SAMPLE_RATE: u32 = 4;
}

struct VelocityLayer;
impl Layer for VelocityLayer {
    type Element = (i8, i8);
    const SAMPLE_RATE: u32 = 1;
    const SWAP_FOLLOW: bool = true;
}
```

---

## Memory Layout

For a 512×512 chunk with 4-byte pixel + heat + pressure:

```
Chunk<GamePixel> (512×512):
┌────────────────────────────────────────────────────────┐
│ pixels: [GamePixel; 262144]                            │  ← 1 MB
│ ┌──────────┬───────┬─────────┬───────┐                 │
│ │ material │ color │ dmg|var │ flags │ × 262144        │
│ └──────────┴───────┴─────────┴───────┘                 │
├────────────────────────────────────────────────────────┤
│ heat: [u8; 16384]  (sample_rate: 4)                    │  ← 16 KB
│ pressure: [u16; 4096]  (sample_rate: 8)                │  ← 8 KB
└────────────────────────────────────────────────────────┘
```

---

## Chunk Structure

```rust
pub struct Chunk<T: PixelData> {
    /// Game-defined pixel data (AoS)
    pub pixels: Surface<T>,

    /// Optional separate layers (SoA)
    /// Game registers these at startup
    layers: LayerStorage,

    /// Metadata
    tile_dirty_rects: Box<[TileDirtyRect]>,
    pos: Option<ChunkPos>,
}

/// Type-erased layer storage (game registers concrete types)
struct LayerStorage {
    data: HashMap<TypeId, Box<dyn Any>>,
}
```

---

## Layer Trait

For separate layers (not the pixel struct):

```rust
/// Separate layer stored as SoA
trait Layer: 'static {
    type Element: Copy + Default;
    const NAME: &'static str;
    const SAMPLE_RATE: u32;  // 1 = per-pixel, 4 = 4×4 regions
}

/// Whether layer data follows pixel swaps
trait SwapFollow: Layer {
    const SWAP_FOLLOW: bool;
}
```

---

## Simulation API

Game implements simulations using its pixel type directly:

```rust
// Game crate - no framework traits required
fn falling_sand_sim(
    world: &mut PixelWorld<GamePixel>,
    materials: &MaterialRegistry,  // Game's material system
    tick: u64,
) {
    // Game accesses pixels directly
    for (pos, pixel) in world.iter_mut_phased(tick) {
        if pixel.is_void() { continue; }

        if let Some(target) = try_fall(pos, world, materials) {
            world.swap(pos, target);
            // Entire GamePixel struct swaps atomically
        }
    }
}
```

The framework provides iteration primitives. The game provides all logic.

---

## Swap Mechanics

Pixel struct swap is a single memory operation:

```rust
impl<T: PixelData> Canvas<T> {
    pub fn swap(&mut self, a: WorldPos, b: WorldPos) {
        // Single memcpy for entire pixel struct
        if a.chunk() == b.chunk() {
            let chunk = self.get_mut(a.chunk());
            chunk.pixels.swap(a.local_index(), b.local_index());
        } else {
            let (ca, cb) = self.get_two_mut(a.chunk(), b.chunk());
            std::mem::swap(
                &mut ca.pixels[a.local_index()],
                &mut cb.pixels[b.local_index()],
            );
        }

        // Also swap any layers marked swap_follow: true
        self.swap_following_layers(a, b);
    }
}
```

**Key properties:**

| Property | Implementation |
|----------|----------------|
| Atomic | Single memory operation for pixel struct |
| No loop | No iteration over fields |
| Cache-friendly | Contiguous memory access |
| Cross-chunk safe | Canvas handles both chunks |

---

## Rendering Integration

Game provides color extraction function:

```rust
// Game crate
fn extract_color(pixel: &GamePixel, palette: &Palette) -> [u8; 4] {
    if pixel.material() == 0 {
        [0, 0, 0, 0]  // transparent void
    } else {
        palette.lookup(pixel.color())
    }
}

// Plugin setup
app.add_plugins(PixelWorldPlugin::<GamePixel>::new(
    config,
    extract_color,  // Framework calls this for GPU upload
));
```

Framework handles dirty tracking and upload scheduling. Game controls color logic.

---

## Plugin Setup

```rust
// Game crate main.rs
fn main() {
    App::new()
        .add_plugins(PixelWorldPlugin::<GamePixel>::new(config, extract_color))

        // Game registers separate layers
        .add_systems(Startup, |mut world: ResMut<PixelWorld<GamePixel>>| {
            world.register_layer::<HeatLayer>();
            world.register_layer::<VelocityLayer>();
        })

        // Game adds its simulations
        .add_systems(FixedUpdate, (
            falling_sand_sim,
            heat_diffusion_sim,
            material_interaction_sim,
        ).chain())

        .run();
}
```

---

## Memory Calculations

| Configuration | Pixel Size | Per Chunk (512²) |
|---------------|------------|------------------|
| 2-byte pixel (minimal) | 2B | 512 KB |
| 4-byte pixel (demo game) | 4B | 1 MB |
| 4-byte pixel + heat + pressure | 4B + layers | ~1 MB |
| 8-byte pixel (extended) | 8B | 2 MB |

Game chooses pixel size based on needs. Framework just stores `T`.

---

## Key Design Decisions

1. **Framework is generic** - stores `T`, knows nothing about pixel internals
2. **Game owns pixel definition** - full control over fields, packing, semantics
3. **Pixel struct swaps atomically** - single memory operation
4. **Separate layers optional** - for data that needs different lifetime/resolution
5. **Game provides color extraction** - framework doesn't know how to render
6. **Demo game as reference** - users clone and modify, not "install and use"

---

## Related Documentation

- [Modularity Refactor](modularity-refactor.md) - Implementation phases
- [Pixel Layers](../arhitecture/modularity/pixel-layers.md) - Layer system design
- [pixel_macro crate](../../crates/pixel_macro/) - POC implementation
