# Project Guidelines

## Testing

- No trivial unit tests. Don't test that getters return what setters set.
- Integration and E2E tests only. Tests should catch real bugs.
- Visual verification via runnable examples is preferred for graphical systems.
- Keep tests in `tests/` directory, not inline `#[cfg(test)]` modules.
- E2E tests: `just test-pixel-world` or `cargo test -p bevy_pixel_world`. Rendering is detected at runtime — no special features needed.

## API Design

- Write only what's needed for the current task. Stop when done.
- Resist completeness. Don't add operations "because a complete API would have them."
- Don't predict the future. Code for hypothetical requirements is usually wrong.
- One working primitive beats many partial ones.

## Code Organization

- Defer abstraction until patterns repeat. Three concrete cases reveal the right abstraction.
- Minimal public surface. Expose only what callers need.

## Conditional Compilation

- Never duplicate functions, types, or entrypoints for `#[cfg]` gating.
- Apply `#[cfg]` to inner fields, statements, and scopes instead.
- One function/type definition with conditional internals, not two definitions with conditional attributes.

## Documentation

- Plans describe *what* to build, not *how*.
- Data structure definitions are permitted in plans. Implementation code is not.
- Use mermaid diagrams for complex systems (state machines, data flow).

## Git Worktrees

Worktrees are **opt-in**. Only create or enter a worktree when the user explicitly asks at the beginning of the conversation. Do not proactively create worktrees.

### If a Worktree Is Requested

1. Establish the worktree before any other work:
   ```bash
   git worktree list
   git worktree add ../sim2d-<descriptive-name> -b <type>/<description>
   cd ../sim2d-<descriptive-name>
   ```

2. **Carry the working directory through the entire session** — all plans, file reads, edits, and commands must use the worktree path, not the main repo path.

3. Plans must include a Working Directory header:
   ```markdown
   ## Working Directory
   `/home/midori/_dev/sim2d-<suffix>` (branch: `<type>/<description>`)
   ```

### Matching Task to Worktree

If a worktree already exists for your task type, use it:
- Docs tasks → worktree with `docs/*` branch
- Refactoring → worktree with `refactor/*` branch
- Feature work → worktree with `feat/*` branch
- Bug fixes → worktree with `fix/*` branch

If no matching worktree exists, create one.

### Creating a Worktree

```bash
# Examples:
git worktree add ../sim2d-arch-docs -b docs/architecture-reorg
git worktree add ../sim2d-plugin-helpers -b refactor/plugin-helpers
git worktree add ../sim2d-physics-desync -b fix/physics-desync-on-load

# After creating the worktree, copy target/ to speed up first compilation:
cp -r target/ ../sim2d-<suffix>/target/
```

> **Tip**: Use `cp -r` (not `cp -al`) so each worktree gets independent files. Hard links would cause lock conflicts when compiling multiple worktrees concurrently. sccache still shares cached artifacts across worktrees.

### Conventions

- **Location**: Sibling directories (`../sim2d-<suffix>`)
- **Descriptive names**: `../sim2d-physics-desync-fix` not `../sim2d-fix`
- **Branch naming**: `<type>/<description>`
- **Cleanup**: `git worktree remove ../sim2d-<suffix>` when merged

## References

See `docs/implementation/methodology.md` for full rationale.
