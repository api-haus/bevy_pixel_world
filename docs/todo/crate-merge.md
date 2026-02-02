# Merge bevy_pixel_world into game crate

Consolidate `crates/bevy_pixel_world/` contents into `crates/game/` to eliminate library separation.

## Context

Following the game-first architecture decision, the separate `bevy_pixel_world` crate no longer serves a purpose. All pixel sandbox code should live in the game crate.

## Tasks

- [ ] Move `crates/bevy_pixel_world/src/` contents to `crates/game/src/pixel_sandbox/` (or similar module)
- [ ] Update all internal imports
- [ ] Update `Cargo.toml` workspace members
- [ ] Remove `crates/bevy_pixel_world/` directory
- [ ] Update any remaining doc references to old crate paths
- [ ] Verify `cargo build` and `cargo test` pass
- [ ] Verify examples still run

## Notes

- Keep module structure logical within game crate
- May want to organize as `src/pixel_sandbox/` submodule for clarity
