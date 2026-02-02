# Material Interactions Phase 7

Implement heat system, material reactions, and additional simulation layers.

## Overview

Add rich material interactions including heat propagation, reactions (corrosion, ignition), decay, and additional simulation layers for moisture and pressure.

## Tasks

### Heat System
- [ ] Implement heat propagation simulation (diffusion)
- [ ] Add heat sources (burning materials, lava)
- [ ] Create heat-based state changes (melting, ignition)
- [ ] Add temperature-based rendering effects (glow)
- [ ] Implement cooling mechanics (water on hot materials)

### Material Reactions
- [ ] Add corrosion system (acid on metals)
- [ ] Implement ignition and burning propagation
- [ ] Create material transformation (water+lava → steam + obsidian)
- [ ] Add reaction triggers (contact, heat, pressure)
- [ ] Implement reaction chains and cascading effects

### Decay and Erosion
- [ ] Add material decay over time (organic materials rot)
- [ ] Implement weathering effects on stone
- [ ] Create erosion from water flow
- [ ] Add rust/corrosion accumulation on metals

### Additional Simulation Layers
- [ ] Implement moisture layer (full resolution, swap-follow)
- [ ] Create pressure layer (4x downsampled, spatial)
- [ ] Add velocity layer for pixel momentum tracking
- [ ] Implement layer interaction rules

### Configuration
- [ ] Add reaction recipes and thresholds
- [ ] Create heat material properties (conductivity, heat capacity)
- [ ] Add decay rates per material

## Technical Details

### Simulation Layers
| Layer | Resolution | Behavior | Example |
|-------|------------|----------|---------|
| Temperature | 1:1 | swap_follow | Hot lava pixel stays hot |
| Heat | 4x downsampled | spatial | Cave region is warm |
| Moisture | 1:1 | swap_follow | Wet sand carries wetness |
| Pressure | 4x downsampled | spatial | Fluid pressure regions |

### Heat Simulation
```rust
fn propagate_heat(chunk: &mut Chunk, heat_layer: &mut HeatLayer) {
    // Diffusion + heat sources
    for cell in heat_layer.cells_mut() {
        let neighbors = cell.neighbors().avg();
        *cell = (*cell + neighbors) / 2.0 * COOLING_FACTOR;
        
        // Add heat from burning pixels
        if chunk.pixel_at(cell.pos).flags.burning {
            *cell += BURN_HEAT;
        }
    }
}
```

## References
- docs/implementation/plan.md (Phase 7)
- docs/architecture/simulation/materials.md
- docs/architecture/modularity/pixel-layers.md
