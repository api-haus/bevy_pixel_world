# POC Implementation Plan: Pixel Sandbox

A demo-first approach delivering visual results at each phase.

## POC Goal

**Deliverable:** Infinite tiled sandbox game where the player:

- Navigates with WASD (no character, free camera)
- Paints materials with cursor (brush size control)
- Sees comprehensive debug overlays (chunk boundaries, dirty rects, tile phases)
- Explores procedurally generated terrain (FastNoise2: air/solid + caves + material layers)

See [methodology.md](methodology.md) for testing and API design principles.
See [plan_history.md](plan_history.md) for archived phases.

---

## Phase Roadmap

| Phase | Focus | Deliverable |
|-------|-------|-------------|
| 0 | Foundational Primitives | *Completed - see plan_history.md* |
| 1 | Rolling Chunk Grid | Coherent supersimplex noise, WASD camera |
| 2 | Material System | Distance-to-surface coloring (soil→stone) |
| 3 | Interaction | Cursor painting materials |
| 4 | Simulation | Cellular automata with 2x2 checkerboard scheduling |

---

## Phase 1: Rolling Chunk Grid

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

**Files:** `pixel_world/src/chunk/pool.rs`

```rust
pub struct ChunkPool {
    free: Vec<Chunk>,
}

impl ChunkPool {
    pub fn new(count: usize) -> Self;
    pub fn acquire(&mut self) -> Option<Chunk>;
    pub fn release(&mut self, chunk: Chunk);
}
```

Uses `CHUNK_SIZE` constant for chunk allocation.

### 1.3 Streaming Window

**Files:** `pixel_world/src/streaming.rs`

```rust
pub struct StreamingWindow {
    active: HashMap<ChunkPos, ActiveChunk>,
    center: ChunkPos,
}

pub struct ActiveChunk {
    pub chunk: Chunk,
    pub entity: Entity,
    pub texture: Handle<Image>,
}
```

The grid maintains a fixed `WINDOW_WIDTH` × `WINDOW_HEIGHT` rectangle of chunks. As the camera moves, chunks roll from one edge to the opposite edge, preserving internal positional consistency.

### 1.4 FastNoise2 Integration

**Dependencies:** `fastnoise2 = "0.4"`

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

- [ ] WASD moves camera smoothly
- [ ] Chunks stream in/out at window edges
- [ ] Noise coherent across boundaries (no seams)
- [ ] Chunk labels visible for debugging

---

## Phase 2: Material System

Build on supersimplex noise with distance-based material coloring.

**Concept:** Color pixels by distance to nearest air (surface)
- Surface pixels → Soil (brown)
- Deeper pixels → Stone (gray)
- Air → transparent/sky blue

**New files:**
- `src/material.rs` - Material enum with color ranges
- `src/pixel.rs` - Pixel struct (material + color variant)

**Algorithm:**
1. Generate noise, threshold to solid/air
2. For each solid pixel, calculate distance to nearest air
3. Map distance to material: 0-N = Soil, N+ = Stone

### Verification

```bash
cargo run -p pixel_world --example rolling_grid
```

- [ ] Surface shows brown soil gradient
- [ ] Interior shows gray stone
- [ ] Smooth color transitions at material boundaries

---

## Phase 3: Interaction

- Cursor world position from screen coords
- Left click: paint selected material
- Right click: erase (set to Air)
- Circular brush with size control
- Simple egui panel for material selection

### Verification

```bash
cargo run -p pixel_world --example painting
```

- [ ] Cursor position tracks correctly at all zoom levels
- [ ] Painting materials updates chunk visuals immediately
- [ ] Brush size slider works
- [ ] Material selector shows available materials

---

## Phase 4: Simulation

Cellular automata with 2x2 checkerboard parallel scheduling:

```
A B A B
C D C D
A B A B
C D C D
```

- Process all A tiles, then B, then C, then D
- Adjacent tiles never same phase (safe parallelism)
- Behaviors: Powder falls, Liquid flows, Solid stays
- Dirty flag optimization

### Verification

```bash
cargo run -p pixel_world --example simulation
```

- [ ] Sand falls and piles at angle of repose
- [ ] Water flows sideways and pools
- [ ] No visible tile seams during simulation
- [ ] Debug overlay shows tile phases

---

## Deferred to Post-POC

- Heat system and heat propagation
- Particle physics (emission, deposition)
- Material interactions (corrosion, ignition, transformation)
- Decay system
- Persistence/saving
- Parallel simulation (rayon)
