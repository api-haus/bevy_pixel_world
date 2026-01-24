# Implementation Plan: Pixel Sandbox

A demo-first approach delivering visual results at each phase.

See [methodology.md](methodology.md) for testing and API design principles.
See [plan_history.md](plan_history.md) for archived phases.

---

## POC Status: Complete ✓

**Delivered:** Infinite tiled sandbox with:

- WASD camera navigation with speed boost
- Cursor painting with material selection and brush size control
- Procedurally generated terrain (FastNoise2 noise seeding)
- Cellular automata simulation (powder falls, liquid flows)
- Debug overlays (chunk boundaries, dirty rects, tile phases)

**Demo:** `cargo run -p pixel_world --example painting`

---

## Phase Roadmap

| Phase | Focus | Status |
|-------|-------|--------|
| 0 | Foundational Primitives | *Completed - see plan_history.md* |
| 1 | Rolling Chunk Grid | *Completed - see plan_history.md* |
| 2 | Material System | *Completed - see plan_history.md* |
| 3 | Interaction | *Completed - see plan_history.md* |
| 4 | Simulation | *Completed - see plan_history.md* |
| 5.0 | Persistence | *Completed - see plan_history.md* |
| 5 | Game Integration | Not started |

---

## Phase 5: Game Integration

Integrate `pixel_world` simulation with the `game` crate player mechanics.

**Goal:** Player interacts with pixel world - collision, digging, building.

### 5.1 Pixel-Player Collision

Player physics body collides with solid/powder pixels.

- Generate collision mesh from pixel data (marching squares)
- Update collision mesh when chunks change
- Player stands on terrain, blocked by walls

### 5.2 Player Tools

- Dig tool: remove pixels in radius around cursor
- Place tool: add selected material pixels
- Tool switching UI

### 5.3 World Interaction

- Player spawn position based on terrain
- Camera follows player (with optional free-cam toggle)

### Acceptance Criteria

- Collision mesh uses Douglas-Peucker simplification (start with 1.0 pixel tolerance, tune based on testing)

### Verification

```bash
cargo run -p game
```

- [ ] Player stands on solid terrain
- [ ] Player blocked by walls
- [ ] Dig tool removes pixels
- [ ] Place tool adds pixels
- [ ] Simulation continues around player

---

## Future Phases (Post-Integration)

### Phase 6: Procedural Generation

- Biomes with different material distributions
- Cave systems and underground features
- Surface features (trees, rocks, structures)
- WFC tile resolution: candidate 64x64 pixels (8 tiles per chunk edge)
- Stamp overlap: priority 0-255 (terrain=0, structures=100, player=255), SDF falloff blending

### Phase 7: Material Interactions

- Heat system and heat propagation
- Material reactions (corrosion, ignition, transformation)
- Decay and erosion
- Additional simulation layers: moisture (full resolution), pressure (4x downsampled like heat)

### Phase 8: Particles

- Particle emission from materials
- Particle deposition back to pixels
- Visual effects (sparks, smoke, debris)
- Rendering: Bevy `Mesh2d` with instance buffer or sprite batching (API research at implementation time)

## Deferred Indefinitely

- Gas physics (rising, dispersal) - complexity vs benefit unclear
- Parallel rayon simulation - current performance adequate
