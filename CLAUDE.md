# Project Guidelines

## Testing

- No trivial unit tests. Don't test that getters return what setters set.
- Integration and E2E tests only. Tests should catch real bugs.
- Visual verification via runnable examples is preferred for graphical systems.
- Keep tests in `tests/` directory, not inline `#[cfg(test)]` modules.

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

## Git Worktrees (Multi-Agent Workflow)

When multiple agents may work concurrently, use git worktrees to prevent interference:

### Worktree Selection

Before starting work, determine task locality from the user's prompt:

1. **Check existing worktrees**: `git worktree list`
2. **Match task to worktree** by examining active branches:
   - Docs tasks → worktree with `docs-*` branch
   - Refactoring → worktree with `refactor-*` branch
   - Feature work → worktree with `feat-*` branch
   - If no matching worktree exists, create one

### Creating a Worktree

```bash
# Create worktree with descriptive name matching the task
git worktree add ../sim2d-<descriptive-name> -b <type>/<description>

# Examples:
git worktree add ../sim2d-arch-docs -b docs/architecture-reorg
git worktree add ../sim2d-plugin-helpers -b refactor/plugin-helpers
git worktree add ../sim2d-physics-desync -b fix/physics-desync-on-load
```

### Worktree Conventions

- **Location**: Sibling directories (`../sim2d-<suffix>`)
- **Descriptive names**: Use specific names, not generic ones. `../sim2d-physics-desync-fix` not `../sim2d-fix`
- **Branch naming**: `<type>/<description>` (e.g., `docs/buoyancy`, `refactor/plugin-helpers`, `fix/physics-desync-on-load`)
- **Cleanup**: Remove worktree when task complete and merged: `git worktree remove ../sim2d-<suffix>`

### Why This Matters

- Each worktree has independent staging area and HEAD
- Prevents one agent's `git add` from polluting another's commit
- sccache shares compiled artifacts across worktrees (configured in `.cargo/config.toml`)

### Single-Agent Exception

If you are certain no other agents are running, you may work directly in the main worktree. When in doubt, use a worktree.

### Plans Must Include Worktree Context

When writing implementation plans, always include the working directory at the top:

```markdown
## Working Directory
`/home/midori/_dev/sim2d-fix` (branch: `fix/physics-desync-on-load`)
```

This ensures agents implementing the plan across context boundaries know which worktree to use.

## References

See `docs/implementation/methodology.md` for full rationale.
