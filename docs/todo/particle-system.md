# Particle System Implementation

Implement the free-form particle system for dynamic effects that complement the cellular automata simulation.

## Overview

Particles handle effects where grid-based simulation is too restrictive:
- **Pouring fluids** - water/lava streams before settling into the grid
- **Explosion debris** - powder materials, dust, sand, fragments ejected by blasts
- **Gases** - smoke, steam, vapor trails with natural movement

Particles and pixels transition bidirectionally: pixels can become particles (emission), and particles can become pixels (deposition).

## Tasks
- [ ] Define `Particle` data structure with position, velocity, material, color
- [ ] Implement particle pool allocator (fixed-size, no heap allocation)
- [ ] Implement pixel → particle emission (explosion, pouring, gas release triggers)
- [ ] Implement particle → pixel deposition (collision detection, adjacent void finding)
- [ ] Implement particle physics update (gravity, drag, material-specific modifiers)
- [ ] Implement rendering with elongated quads for motion blur effect
- [ ] Add particle pass to simulation scheduling (after CA phases)
- [ ] Add integration with explosion/bomb system for debris emission
- [ ] Add pouring mechanics for liquid materials

## Technical Details

### Particle Data Structure
```rust
struct Particle {
    position: (f32, f32),  // world coordinates, sub-pixel precision
    velocity: (f32, f32),  // movement per tick
    material: u8,          // same material ID as pixels
    color: u8,             // palette index for rendering
}
```

### Rendering Approach
Use Bevy `Mesh2d` with instance buffer or sprite batching (API research at implementation time).

### Scheduling
```
CA Simulation (4 phases) → Particle Update → Material Interactions
```

## References
- docs/architecture/simulation/particles.md
- Phase 8 of implementation plan (deferred features)
