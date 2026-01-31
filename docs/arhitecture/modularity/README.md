# Modularity

Extensibility architecture for rendering backends, pixel layers, and simulation rules.

## Overview

The pixel sandbox is designed as an extensible library, not a closed application. Crate consumers can:

- Swap rendering backends (GPU, terminal, headless)
- Add custom layers at any resolution (per-pixel or downsampled)
- Implement custom simulation rules using provided building blocks

## Documentation

| Document | Purpose |
|----------|---------|
| [Rendering Backends](rendering-backends.md) | Abstract render targets for different output modes |
| [Pixel Layers](pixel-layers.md) | Unified layer system with sample rate, swap-follow, render modularity |
| [Simulation Extensibility](simulation-extensibility.md) | Pluggable simulation rules and reusable library functions |

## Design Principles

**Stable core, extensible surface:**

- Base pixel format is an immutable contract
- Extension points have well-defined interfaces
- Breaking changes require major version bumps

**Composition over inheritance:**

- Traits define extension points
- Library functions provide reusable primitives
- Consumers compose custom behavior from provided building blocks

**Zero-cost abstractions:**

- Trait-based dispatch compiles to direct calls
- Extension layers use SoA for cache efficiency
- Optional features don't impose overhead when unused

## Related Documentation

- [Pixel Format](../foundational/pixel-format.md) - Base pixel data structure
- [Simulation](../simulation/simulation.md) - Core simulation passes
- [Rendering](../rendering/rendering.md) - Current GPU rendering pipeline
- [Architecture Overview](../README.md)
