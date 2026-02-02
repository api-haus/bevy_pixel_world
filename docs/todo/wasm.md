# WASM Tasks

Platform-specific work for WebAssembly builds.

## Persistence

- [ ] Implement WASM persistence request tracking (native has full tracking)
- [ ] Track completion status of OPFS save operations
- [ ] Handle multiple concurrent save requests on WASM

## Performance

- [ ] Profile WASM build for bottlenecks
- [ ] Evaluate SIMD usage via wasm-simd feature

## References

- docs/refactoring/07-wasm-patterns.md (reference documentation)
- crates/game/src/world/persistence_systems.rs:408
