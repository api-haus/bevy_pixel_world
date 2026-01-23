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
| 1 | Rolling Chunk Grid | *Completed - see plan_history.md* |
| 2 | Material System | Distance-to-surface coloring (soil→stone) |
| 3 | Interaction | Cursor painting materials |
| 4 | Simulation | Cellular automata with 2x2 checkerboard scheduling |

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

**New types in `pixel_world/src/coords.rs`:**

```rust
/// Material registry index (0-255).
/// See `docs/arhitecture/materials.md` for the material system.
pub struct MaterialId(pub u8);

/// Palette color index (0-255).
/// See `docs/arhitecture/pixel-format.md` for color field usage.
pub struct ColorIndex(pub u8);
```

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
