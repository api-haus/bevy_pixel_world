# CLAUDE.md

## Philosophy

- **YAGNI.** Write only what's needed now. Resist "completeness."
- **Rule of three.** Don't abstract until you've seen the pattern three times.
- **One working primitive > many partial ones.**

## Testing

- No trivial unit tests. Integration/E2E only — tests should catch real bugs.
- Visual verification via runnable examples preferred for graphics.
- Tests live in `tests/`, not inline `#[cfg(test)]` modules.
- Run: `just test-pixel-world` or `cargo test -p bevy_pixel_world`

## Code Style

### Conditional Compilation
Never duplicate functions/types for `#[cfg]`. Apply to inner fields/statements instead:

```rust
// ✓ Good
app.add_systems(Update, (
    #[cfg(target_arch = "wasm32")]
    wasm_only_system,
    always_runs,
).chain());

// ✗ Bad — duplicated call
#[cfg(target_arch = "wasm32")]
app.add_systems(Update, (wasm_only_system, always_runs).chain());
#[cfg(not(target_arch = "wasm32"))]
app.add_systems(Update, always_runs);
```

### Cargo Features
This is a game, not a library. Don't add Cargo features. Valid exceptions:
- Platform splits (`wasm32` vs native)
- Truly optional heavy dependencies with a concrete use case

Core functionality (rendering, camera, input) compiles unconditionally.

### API Surface
Minimal public exposure. Only what callers actually need.

## Documentation

- Plans describe *what*, not *how*. Data structures OK, implementation code not.
- Use mermaid for complex flows (state machines, data flow).

## Git

**Never push.** User pushes after review.

### Worktrees

When working in a worktree, include its path in plan mode headers.

### Submodules

After merging changes to submodules, run `git submodule update --init` to checkout content.

---

*See `docs/implementation/methodology.md` for rationale.*
