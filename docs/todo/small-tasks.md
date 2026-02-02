# Small Tasks

Miscellaneous improvements and fixes.

## Code Quality

- [ ] Run `cargo clippy -p bevy_pixel_world -- -D warnings` and fix all warnings
- [ ] Run `debtmap analyze crates/bevy_pixel_world --threshold-complexity 10` and address high-complexity functions
- [ ] Review WASM compatibility patterns (07-wasm-patterns.md)

## Known TODOs in Code

- [ ] Dirty rects stability with jitter enabled (simulation/mod.rs:54)
- [ ] Copy-on-write target_path requires IoDispatcher CopyTo command (world/control.rs:191)
- [ ] WASM persistence request tracking not implemented (world/persistence_systems.rs:408)

## Documentation

- [ ] Update docs/architecture if any drift from implementation
- [ ] Ensure all public APIs have doc comments

## Testing

- [ ] Add integration test for chunk persistence round-trip
- [ ] Add integration test for streaming window chunk lifecycle
- [ ] Visual verification example for collision mesh generation
