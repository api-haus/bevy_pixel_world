# Internal Modularity

> **Status: Deferred**
>
> These patterns may be useful for future refactoring to reduce coupling and improve maintainability. Not actively planned.

Internal organization patterns for minimizing refactoring impact when pixel sandbox evolves.

---

## Optional Traits

Abstraction over pixel internals for systems that need specific capabilities:

```rust
/// For collision mesh generation (marching squares)
pub trait PixelCollision {
    fn is_solid(&self) -> bool;
}

/// For dirty-based simulation scheduling
pub trait PixelDirty {
    fn is_dirty(&self) -> bool;
    fn set_dirty(&mut self, dirty: bool);
}
```

Systems using these traits don't depend on specific pixel field layout.

---

## Generic Storage

Storage types could be made generic to decouple from pixel struct:

```rust
pub struct Chunk<T: Copy + Default + 'static> { data: Surface<T>, ... }
pub struct Canvas<T: Copy + Default + 'static> { ... }
```

**Benefit:** Storage logic doesn't need changes when pixel fields change.

**Current state:** Hardcoded `Pixel` type - acceptable while iterating on design.

---

## Separate Layers (SoA)

Data with different resolution or lifetime can live in separate arrays:

| Layer | Resolution | swap_follow | Purpose |
|-------|------------|-------------|---------|
| Heat | 1/4 | false | Thermal diffusion (spatial) |
| Velocity | 1:1 | true | Pixel momentum (moves with pixel) |

See [Pixel Layers](../architecture/modularity/pixel-layers.md) for design details.

---

## When to Apply

Consider these patterns when:
- Changing pixel fields requires touching many unrelated systems
- Adding new data types (layers) requires invasive changes
- Testing storage logic separately from simulation logic becomes valuable

Don't apply prematurely - current hardcoded approach is fine while design stabilizes.

---

## Related Documentation

- [Pixel Layers](../architecture/modularity/pixel-layers.md) - Layer system architecture
- [Simulation Extensibility](../architecture/modularity/simulation-extensibility.md) - Simulation patterns
