# Modularity Refactor Tasks

From docs/implementation/modularity-refactor.md. Goal: generic spatial infrastructure, game defines pixel/material/simulation.

## Phase M0: Audit

- [ ] Document all framework->pixel coupling points
- [ ] Categorize: storage (keep generic), iteration (keep), rendering (keep), pixel access (move), material (move), simulation (move)

## Phase M1: Generic Chunk

- [ ] Make `Chunk<T>` generic over pixel type
- [ ] Make `Canvas<T>` generic
- [ ] Make `PixelWorld<T>` generic
- [ ] Make `PixelWorldPlugin<T>` generic

## Phase M2: Extract Pixel

- [ ] Move `Pixel` struct to game crate
- [ ] Move `PixelFlags` to game crate
- [ ] Define optional traits `PixelCollision`, `PixelDirty`

## Phase M3: Extract Material

- [ ] Move material system to game crate
- [ ] Remove material dependency from framework

## Phase M4: Extract Simulation

- [ ] Move falling simulation to game
- [ ] Move burning simulation to game
- [ ] Move heat simulation to game
- [ ] Keep iteration/scheduling infrastructure in framework

## Phase M5: Rendering Generics

- [ ] Raw pixel upload (no CPU color transform)
- [ ] Shader interprets pixel bytes directly
- [ ] Palette LUT utilities

## Phase M6: Polish

- [ ] Demo game as reference implementation
- [ ] Documentation updates
