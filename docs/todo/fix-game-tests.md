# Fix game crate tests

The game crate tests are broken after the bevy_pixel_world merge. Tests need full evaluation and fixes.

## Problems

1. **No library target** - Tests use `use game::...` but crate has no `[lib]`
2. **Type inference issues** - Some tests have broken type inference (gremlins_stress.rs)
3. **Stale avian2d references** - Some test files still reference removed avian2d feature

## Tasks

- [ ] Add `src/lib.rs` that re-exports modules needed by tests
- [ ] Add `[lib]` section to Cargo.toml
- [ ] Update `main.rs` to use the library
- [ ] Remove remaining avian2d references from test files
- [ ] Fix type inference issues in gremlins_stress.rs
- [ ] Fix any other test compilation errors
- [ ] Run full test suite: `cargo test -p game`
- [ ] Verify all tests pass (not just compile)

## Affected Tests

- submergence_e2e.rs - avian2d references removed, needs lib.rs
- spawn_pixel_body_e2e.rs - still has avian2d cfg blocks
- persistence_e2e.rs - needs lib.rs
- body_stability_e2e.rs - needs lib.rs
- gremlins_stress.rs - type inference error on `world.set_heat_at()`
- body_persistence_e2e.rs - needs evaluation
- All other tests in `tests/pixel_world/`
