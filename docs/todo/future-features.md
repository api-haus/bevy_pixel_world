# Future Features

From implementation plan phases 6-8.

## Phase 6: Procedural Generation

- [ ] Biomes with different material distributions
- [ ] Cave systems and underground features
- [ ] Surface features (trees, rocks, structures)
- [ ] WFC tile resolution (64x64 candidates)
- [ ] Stamp overlap with priority system

See docs/todo/procedural-generation-phase6.md for detailed tasks.

## Phase 7: Material Interactions

- [ ] Heat system expansion (beyond current 16x16 downsampled grid)
- [ ] Material reactions (corrosion, ignition, transformation)
- [ ] Decay and erosion
- [ ] Moisture layer (full resolution)
- [ ] Pressure layer (4x downsampled)

## Phase 8: Particles

- [ ] Particle emission from materials
- [ ] Particle deposition back to pixels
- [ ] Visual effects (sparks, smoke, debris)
- [ ] Bevy Mesh2d instance buffer or sprite batching

See docs/todo/particle-system.md for detailed tasks.

## Deferred Indefinitely

- Gas physics (rising, dispersal) - complexity vs benefit unclear
- Parallel rayon simulation - current performance adequate
