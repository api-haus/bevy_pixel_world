# Implementation Plan History

Archived phases from `plan.md`.

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
