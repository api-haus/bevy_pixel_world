# Refactoring Opportunities

Self-contained refactoring documents for `bevy_pixel_world`. Each can be tackled independently.

## Documents

| # | Document | Risk | Description |
|---|----------|------|-------------|
| 01 | [Clippy Lint Fixes](01-clippy-lint-fixes.md) | None | Mechanical fixes for Clippy warnings |
| 02 | [Bomb Complexity](02-bomb-complexity.md) | Low | Decompose `compute_bomb_shell()` |
| 03 | [Heat Complexity](03-heat-complexity.md) | Low | Reduce complexity in heat simulation |
| 04 | [Blast Complexity](04-blast-complexity.md) | Low | Decompose `blast()` function |
| 05 | [Module Visibility](05-module-visibility.md) | Medium | Clean up public API surface |
| 06 | [WorldSave SRP](06-world-save-srp.md) | High | Extract persistence concerns (deferred) |
| 07 | [WASM Patterns](07-wasm-patterns.md) | — | Reference documentation (no changes) |

## Recommended Order

1. **01-clippy-lint-fixes** — Quick wins, immediate CI benefit
2. **05-module-visibility** — Low risk, clarifies API boundaries
3. **02, 03, 04** — Complexity reductions, any order
4. **06** — Defer for dedicated session

## Verification Commands

After each refactoring:

```bash
cargo clippy -p bevy_pixel_world -- -D warnings
cargo build -p bevy_pixel_world
cargo test -p bevy_pixel_world
```

For complexity measurement:

```bash
debtmap analyze crates/bevy_pixel_world --threshold-complexity 10
```
