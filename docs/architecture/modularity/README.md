# Modularity

> **Status: Planned Architecture**
>
> This describes target modularity. Current implementation uses hardcoded `Pixel` struct. The layer system described in sub-documents is not yet implemented.

Internal organization for custom layers and simulation flexibility.

## Goals

- **Custom layers** at any resolution (per-pixel or downsampled)
- **Swappable simulation schedules** using Bevy's system model
- **Reusable iteration primitives** (checkerboard phasing, dirty tracking)

## Documentation

| Document | Purpose |
|----------|---------|
| [Pixel Layers](pixel-layers.md) | Unified layer system with swap-follow and render modularity |
| [Simulation Extensibility](simulation-extensibility.md) | Simulation rules and shared functions |

## Design Principles

**Composition over inheritance:**

- Traits define extension points
- Shared functions provide reusable primitives
- Custom behavior composed from building blocks

**Zero-cost abstractions:**

- Trait-based dispatch compiles to direct calls
- Extension layers use SoA for cache efficiency
- Optional features don't impose overhead when unused

## Related Documentation

- [Pixel Format](../foundational/pixel-format.md) - Base pixel data structure
- [Simulation](../simulation/simulation.md) - Core simulation passes
- [Rendering](../rendering/rendering.md) - GPU rendering pipeline
- [Architecture Overview](../README.md)
