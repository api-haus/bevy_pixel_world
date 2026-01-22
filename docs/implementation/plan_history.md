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
