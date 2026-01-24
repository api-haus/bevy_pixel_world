# Implementation Plan History

Archived phases from `plan.md`. Phases are moved here upon completion.

---

## POC Phases (All Completed)

---

## Phase 0: Foundational Primitives (Completed)

The foundation is a **Surface** (blittable pixel buffer) and a **Chunk** (container for surfaces). Validated by
rendering a UV-colored quad at 60 TPS.

### 0.1: Surface (Blittable Pixel Buffer)

A generic 2D buffer of elements that can be written to.

**Files:** `pixel_world/src/surface.rs`

```rust
pub struct Surface<T> {
    data: Box<[T]>,
    width: u32,
    height: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

pub type RgbaSurface = Surface<Rgba>;
```

**API:** `new`, `get`, `set`, `width`, `height`, `as_bytes` (for GPU upload)

**Acceptance Criteria:**

- [x] Index calculation: `y * width + x`
- [x] Out-of-bounds returns `None`/`false` (no panic)
- [x] `as_bytes()` returns contiguous slice for GPU upload

---

### 0.2: Blitter (Surface Drawing API)

Fragment-shader-style API for writing into surfaces.

**Files:** `pixel_world/src/blitter.rs`

```rust
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

pub struct Blitter<'a, T> {
    surface: &'a mut Surface<T>,
}
```

**API:**

- `blit(rect, |x, y, u, v| -> T)` - iterate rect, call closure with absolute coords (x,y) and normalized coords (u,v
  0.0-1.0)
- `fill(rect, value)` - solid fill
- `clear(value)` - clear entire surface

**Acceptance Criteria:**

- [x] `blit()` provides correct (x, y, u, v) to closure
- [x] Rect outside bounds is clamped (partial draw, no panic)

---

### 0.3: Chunk (Container)

A spatial unit containing surfaces.

**Files:** `pixel_world/src/chunk.rs`

```rust
pub struct Chunk {
    pub pixels: RgbaSurface,
}
```

---

### 0.4: Texture Upload & Display

Bevy integration for GPU rendering.

**Files:** `pixel_world/src/render.rs`

**API:**

- `create_texture(images, width, height)` - create RGBA8 texture with nearest-neighbor sampling
- `upload_surface(surface, image)` - copy surface bytes to texture

---

### 0.5: 60 TPS UV Quad Demo

**Files:** `pixel_world/examples/uv_quad.rs`

Bevy app that blits an animated UV-colored quad into a chunk at 60 TPS. The quad bounces around with a pulsing blue
channel.

**Verification:** `cargo run -p pixel_world --example uv_quad`

- [x] UV quad displays with correct gradient (red→right, green→down)
- [x] Animation runs at stable 60 TPS
- [x] Blue channel pulses over time

---

## Phase 1: Rolling Chunk Grid (Completed)

**Deliverable:** `cargo run -p pixel_world --example rolling_grid`

### 1.1 Constants & Coordinate Types

**Files:** `pixel_world/src/coords.rs`

```rust
// Compile-time constants - never passed as arguments
pub const CHUNK_SIZE: u32 = 512;
pub const TILE_SIZE: u32 = 16;
pub const WINDOW_WIDTH: u32 = 6;   // chunks horizontally
pub const WINDOW_HEIGHT: u32 = 4;  // chunks vertically (landscape orientation)

// Derived constants - expressed as formulas, not magic numbers
pub const POOL_SIZE: usize = (WINDOW_WIDTH * WINDOW_HEIGHT) as usize;
pub const TILES_PER_CHUNK: u32 = CHUNK_SIZE / TILE_SIZE;

pub struct WorldPos(pub i64, pub i64);   // global pixel
pub struct ChunkPos(pub i32, pub i32);   // chunk grid
pub struct LocalPos(pub u16, pub u16);   // pixel within chunk

impl WorldPos {
    pub fn to_chunk_and_local(self) -> (ChunkPos, LocalPos);
}
```

Floor division for negative coords (not truncation).

### 1.2 Chunk Pool

**Files:** `pixel_world/src/streaming/pool.rs`

```rust
pub struct ChunkPool {
    slots: Vec<PoolSlot>,
}

impl ChunkPool {
    pub fn new() -> Self;
    pub fn acquire(&mut self) -> Option<PoolHandle>;
    pub fn release(&mut self, handle: PoolHandle);
    pub fn get(&self, handle: PoolHandle) -> &Chunk;
    pub fn get_mut(&mut self, handle: PoolHandle) -> &mut Chunk;
}
```

Uses `POOL_SIZE` constant for pre-allocation.

### 1.3 Streaming Window

**Files:** `pixel_world/src/streaming/window.rs`

```rust
pub struct StreamingWindow {
    active: HashMap<ChunkPos, ActiveChunk>,
    center: ChunkPos,
    pool: ChunkPool,
}

pub struct ActiveChunk {
    pub handle: PoolHandle,
    pub entity: Entity,
    pub texture: Handle<Image>,
    pub dirty: bool,
}
```

The grid maintains a fixed `WINDOW_WIDTH` × `WINDOW_HEIGHT` rectangle of chunks. As the camera moves, chunks roll from one edge to the opposite edge, preserving internal positional consistency.

### 1.4 FastNoise2 Integration

**Dependencies:** `fastnoise2 = "0.4"`

**Files:** `pixel_world/src/seeding/mod.rs`, `pixel_world/src/seeding/noise.rs`

```rust
pub trait ChunkSeeder {
    fn seed(&self, pos: ChunkPos, chunk: &mut Chunk);
}

pub struct NoiseSeeder {
    node: SafeNode,
    scale: f32,
}
```

Terrain fill using `SuperSimplex` node:
- World coords = `chunk_pos * chunk_size + local`
- Coherent across chunk boundaries (no seams)
- Output: grayscale noise (threshold for solid/air)

### 1.5 WASD Camera

- WASD/Arrows: move camera
- Shift: speed boost
- Camera position drives StreamingWindow updates

### Verification

```bash
cargo run -p pixel_world --example rolling_grid
```

- [x] WASD moves camera smoothly
- [x] Chunks stream in/out at window edges
- [x] Noise coherent across boundaries (no seams)
- [x] Chunk labels visible for debugging

---

## Phase 2: Material System (Completed)

Material definitions with physics states and color palettes.

### 2.1 Material Types

**Files:** `pixel_world/src/material.rs`

```rust
pub enum PhysicsState {
    Solid,   // Does not move
    Powder,  // Falls, piles, slides
    Liquid,  // Falls, flows horizontally
    Gas,     // Rises, disperses (deferred)
}

pub struct Material {
    pub name: &'static str,
    pub palette: [Rgba; 8],      // 8-color gradient
    pub state: PhysicsState,
    pub density: u8,             // For displacement
    pub dispersion: u8,          // Horizontal spread (liquids)
    pub air_resistance: u8,      // 1/N skip chance
    pub air_drift: u8,           // 1/N drift chance
}
```

### 2.2 Pixel Format

**Files:** `pixel_world/src/pixel.rs`

4-byte cache-efficient pixel format:

```rust
pub struct Pixel {
    pub material: MaterialId,  // u8
    pub color: ColorIndex,     // u8 (palette index)
    pub damage: u8,
    pub flags: u8,             // DIRTY, SOLID, FALLING
}
```

### 2.3 Built-in Materials

- **Air** - Transparent, density 0
- **Soil** - Brown gradient, Powder, density 150
- **Stone** - Gray gradient, Solid, density 200
- **Sand** - Tan gradient, Powder, density 160
- **Water** - Blue gradient, Liquid, density 100

### Verification

- [x] Materials have distinct visual palettes
- [x] Physics states correctly assigned
- [x] Density values support displacement logic

---

## Phase 3: Interaction (Completed)

Cursor-based painting with material selection UI.

**Files:** `pixel_world/examples/painting.rs`

### 3.1 Features

- Cursor world position from screen coords via `Camera::viewport_to_world_2d`
- Left click: paint selected material
- Right click: erase (set to Air)
- Circular brush with configurable radius
- Scroll wheel adjusts brush size
- egui side panel for material selection

### 3.2 Brush System

```rust
struct BrushState {
    radius: u32,           // 2-100 pixels
    painting: bool,
    erasing: bool,
    world_pos: Option<(i64, i64)>,
    material: MaterialId,
}
```

Uses `PixelWorld::blit()` API for efficient parallel painting.

### Verification

```bash
cargo run -p pixel_world --example painting
```

- [x] Cursor position tracks correctly at all zoom levels
- [x] Painting materials updates chunk visuals immediately
- [x] Brush size slider works (egui)
- [x] Scroll wheel adjusts brush size
- [x] Material selector shows available materials

---

## Phase 4: Simulation (Completed)

Cellular automata with checkerboard parallel scheduling.

**Files:**
- `pixel_world/src/simulation/mod.rs` - Tick orchestration
- `pixel_world/src/simulation/physics.rs` - Movement rules
- `pixel_world/src/scheduling/blitter.rs` - Parallel tile processing

### 4.1 Checkerboard Scheduling

```
A B A B
C D C D
A B A B
C D C D
```

Process phases sequentially (A→B→C→D), tiles within each phase in parallel.
Adjacent tiles never share phase, ensuring thread-safe pixel access.

### 4.2 Physics Rules

**Powder (sand, soil):**
1. Air resistance check (1/N skip chance)
2. Try falling straight down (with optional drift)
3. Try sliding diagonally left/right

**Liquid (water):**
1. Air resistance check
2. Try falling (with drift)
3. Try sliding diagonally
4. Try horizontal flow (dispersion)

**Density displacement:** Heavier materials sink into lighter liquids.

### 4.3 Deterministic Randomness

```rust
pub struct SimContext {
    pub seed: u64,      // World seed
    pub tick: u64,      // Current tick
    pub jitter_x: i64,  // Tile grid offset
    pub jitter_y: i64,
}
```

Hash function provides per-pixel, per-tick randomness for natural behavior.

### Verification

```bash
cargo run -p pixel_world --example painting
```

- [x] Sand falls and piles at angle of repose
- [x] Water flows sideways and pools
- [x] No visible tile seams during simulation
- [x] Density displacement works (sand sinks in water)

---

## Phase 5.0: Persistence (Completed)

Chunk serialization with LZ4 compression and streaming integration.

**Files:**
- `pixel_world/src/persistence/mod.rs` - Core persistence API
- `pixel_world/src/persistence/format.rs` - File format (header, page table, data region)
- `pixel_world/src/persistence/compression.rs` - LZ4 and delta compression
- `pixel_world/tests/persistence_e2e.rs` - End-to-end tests
- `docs/arhitecture/chunk-persistence.md` - Full specification

### 5.0.1 Three-State Dirty Tracking

Chunks track modification state to determine save behavior:

| State     | Description                       | On Recycle        |
|-----------|-----------------------------------|-------------------|
| Clean     | Matches procedural generation     | Skip save         |
| Modified  | Changed since load/seed           | Save required     |
| Persisted | Saved to disk, not modified since | Skip save         |

The `modified` flag tracks simulation changes (pixels moved). The `dirty` flag tracks rendering updates. These are separate concerns - a chunk can need rendering without needing persistence.

### 5.0.2 Delta Compression Strategy

Delta vs full storage decision based on modification density:

- **Delta storage**: Only changed pixels stored as `(position, new_pixel)` pairs
- **Full storage**: Complete chunk buffer when delta would be larger
- **Threshold**: 75% modification density - below this, delta wins

Delta compression achieves 100-1000x better compression for lightly modified chunks.

### 5.0.3 Streaming Integration

Persistence hooks into chunk lifecycle:

1. **On chunk recycle**: If modified, auto-save before returning to pool
2. **On chunk load**: Check persistence index before procedural seeding
3. **Background I/O**: Disk operations don't block simulation

### 5.0.4 File Format Summary

Single file with three regions:

1. **Header** (64 bytes): Magic, version, world seed, chunk count
2. **Page Table**: Sorted array of `(ChunkPos, offset, size, storage_type)` entries
3. **Data Region**: LZ4-compressed chunk data, variable length

### Verification

```bash
cargo test --test persistence_e2e
```

- [x] Chunks save when modified and recycled
- [x] Chunks load from disk correctly
- [x] Delta compression works for sparse modifications
- [x] Full compression falls back when delta is larger
- [x] Clean chunks skip persistence
