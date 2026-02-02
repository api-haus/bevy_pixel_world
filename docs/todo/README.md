# Todo Overview

Task tracking for sim2d development. Each file focuses on a specific area.

## Active Development (Phases 5.3-5.5)

| File | Status | Priority |
|------|--------|----------|
| [player-integration.md](player-integration.md) | Phase 5.3 done, 5.4-5.5 pending | **High** |
| [player-dig-place-tools.md](player-dig-place-tools.md) | Phase 5.4 not started | High |
| [free-camera-toggle.md](free-camera-toggle.md) | Phase 5.5 partial | Medium |

## Infrastructure

| File | Status | Priority |
|------|--------|----------|
| [crate-merge.md](crate-merge.md) | Not started | **High** |
| [refactoring.md](refactoring.md) | Summary of refactoring tasks | Low |
| [small-tasks.md](small-tasks.md) | Misc fixes and quality | Low |

## Refactoring (Detailed)

| File | Risk | Description |
|------|------|-------------|
| [clippy-lint-fixes.md](clippy-lint-fixes.md) | None | Mechanical Clippy warning fixes |
| [bomb-complexity-reduction.md](bomb-complexity-reduction.md) | Low | Decompose `compute_bomb_shell()` |
| [heat-complexity-reduction.md](heat-complexity-reduction.md) | Low | Reduce heat simulation complexity |
| [blast-complexity-reduction.md](blast-complexity-reduction.md) | Low | Decompose `blast()` function |

## Persistence

| File | Status | Priority |
|------|--------|----------|
| [named-saves.md](named-saves.md) | Partial (save() works, copy-on-write TODO) | Medium |
| [recovery-persistence.md](recovery-persistence.md) | Autosave exists, dual-save TODO | Medium |
| [wasm.md](wasm.md) | WASM-specific gaps | Low |

## Future Features (Phases 6-8)

| File | Status | Priority |
|------|--------|----------|
| [future-features.md](future-features.md) | Phase 6-8 overview | Deferred |
| [procedural-generation-phase6.md](procedural-generation-phase6.md) | Phase 6 detailed | Deferred |
| [material-interactions-phase7.md](material-interactions-phase7.md) | Phase 7 detailed | Deferred |
| [particle-system.md](particle-system.md) | Phase 8 detailed | Deferred |

## Quick Reference

**Next actionable tasks:**
1. Merge bevy_pixel_world into game crate
2. Phase 5.4: Player dig/place tools
3. Phase 5.5: Free-cam toggle

**Known code TODOs:**
- `simulation/mod.rs:54` - Dirty rects stability with jitter
- `world/control.rs:191` - Copy-on-write target_path
- `world/persistence_systems.rs:408` - WASM persistence tracking
