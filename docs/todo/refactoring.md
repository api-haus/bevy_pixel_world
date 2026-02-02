# Refactoring Tasks

Summary of refactoring opportunities from docs/refactoring/. See detailed task files below.

## Quick Wins (Low Risk)

- [ ] [clippy-lint-fixes.md](clippy-lint-fixes.md): Mechanical Clippy warning fixes
- [ ] [module-visibility-cleanup.md](module-visibility-cleanup.md): Clean up public API surface

## Complexity Reductions (Low Risk)

- [ ] [bomb-complexity-reduction.md](bomb-complexity-reduction.md): Decompose `compute_bomb_shell()`
- [ ] [heat-complexity-reduction.md](heat-complexity-reduction.md): Reduce complexity in heat simulation
- [ ] [blast-complexity-reduction.md](blast-complexity-reduction.md): Decompose `blast()` function

## Deferred (High Risk)

- [ ] 06-world-save-srp: Extract persistence concerns from WorldSave (no detailed task file yet)

## Reference Only

- 07-wasm-patterns: Reference documentation (no changes needed)

## Recommended Order

1. clippy-lint-fixes — Quick wins, immediate CI benefit
2. module-visibility-cleanup — Clarifies API boundaries
3. bomb/heat/blast complexity — Any order
4. world-save-srp — Defer for dedicated session

## Verification

```bash
cargo clippy -p game -- -D warnings
cargo build -p game
cargo test -p game
debtmap analyze crates/game --threshold-complexity 10
```

## References

- docs/refactoring/README.md - Source refactoring documentation
