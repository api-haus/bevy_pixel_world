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

**Demo:** `cargo run -p bevy_pixel_world --example painting`

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
| 5.1 | Pixel Bodies | *Completed - see plan_history.md* |
| 5.2 | Editor Integration | *Completed - see plan_history.md* |
| 5.3 | Player-World Collision | In Progress |
| 5.4 | Player Tools (Dig/Place) | Not started |

---

## Phase 5.3: Player-World Collision

Enable player physics body to collide with pixel terrain.

**Status:** In Progress - collision mesh generation exists, needs integration

### Tasks

- [ ] Generate collision mesh from chunk pixel data (marching squares)
- [ ] Update collision mesh when chunks change (dirty tracking)
- [ ] Integrate with player movement controller
- [ ] Douglas-Peucker simplification (start with 1.0 pixel tolerance)

### Verification

```bash
cargo run -p game
```

- [ ] Player stands on solid terrain
- [ ] Player blocked by walls
- [ ] Can walk on pixel bodies

---

## Phase 5.4: Player Tools

Add dig/place tools for player interaction with pixel world.

**Status:** Not started

### Tasks

- [ ] Dig tool: remove pixels in radius around cursor
- [ ] Place tool: add selected material pixels
- [ ] Tool switching UI (or keybinds)
- [ ] Tool range indicator

### Verification

- [ ] Left click digs pixels
- [ ] Right click places pixels
- [ ] Tool radius adjustable

---

## Phase 5.5: Camera & Spawn

Camera follows player with free-cam toggle.

**Status:** Partial - editor integration complete

### Tasks

- [x] Player spawn at spawn point (editor integration)
- [ ] Camera follows player in play mode
- [ ] Free-cam toggle (for creative mode)

### Verification

- [x] Player spawns at editor-defined spawn point
- [ ] Camera tracks player movement

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
