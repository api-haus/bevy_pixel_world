# Project Guidelines

## Testing

- No trivial unit tests. Don't test that getters return what setters set.
- Integration and E2E tests only. Tests should catch real bugs.
- Visual verification via runnable examples is preferred for graphical systems.
- Keep tests in `tests/` directory, not inline `#[cfg(test)]` modules.
- E2E tests require headless mode: `just test-pixel-world` or `cargo test -p bevy_pixel_world --features headless --no-default-features`. Tests will fail without `--no-default-features` due to missing GPU resources.

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

**MANDATORY**: Always work in a dedicated worktree, never directly in main.

### First Step: Establish Worktree

Before doing ANY other work, you MUST establish a worktree:

```bash
# 1. Check existing worktrees
git worktree list

# 2. Either cd to a matching worktree, or create one:
git worktree add ../sim2d-<descriptive-name> -b <type>/<description>
cd ../sim2d-<descriptive-name>
```

Do not read files, do not explore the codebase, do not make plans—establish your worktree FIRST.

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
```

### Conventions

- **Location**: Sibling directories (`../sim2d-<suffix>`)
- **Descriptive names**: `../sim2d-physics-desync-fix` not `../sim2d-fix`
- **Branch naming**: `<type>/<description>`
- **Cleanup**: `git worktree remove ../sim2d-<suffix>` when merged

### Why This Is Mandatory

- You cannot know if other agents are running
- Main worktree may have uncommitted changes from other work
- Each worktree has independent staging area and HEAD
- sccache shares compiled artifacts (no rebuild penalty)

### Plans Must Include Worktree Context

When writing implementation plans, always include the working directory at the top:

```markdown
## Working Directory
`/home/midori/_dev/sim2d-fix` (branch: `fix/physics-desync-on-load`)
```

This ensures agents implementing the plan across context boundaries know which worktree to use.

## References

See `docs/implementation/methodology.md` for full rationale.
