---
name: worktree
description: Create or switch to a git worktree for isolated feature/fix development
---

Create a new git worktree or switch to an existing one. Only use when explicitly requested.

## Behavior

- If the worktree already exists → switch to it
- If the worktree doesn't exist → create it, then switch to it

## Naming Convention

| Component | Format | Example |
|-----------|--------|---------|
| Todo file | `docs/todo/<slug>.md` | `docs/todo/player-collision.md` |
| Worktree | `../sim2d-<slug>` | `../sim2d-player-collision` |
| Branch | `<type>/<slug>` | `feat/player-collision` |

The `<slug>` must be identical across all three for `/merge` cleanup to work.

## Usage

```bash
# Check if worktree exists
if [ -d "../sim2d-<slug>" ]; then
    cd ../sim2d-<slug>
else
    git worktree add ../sim2d-<slug> -b <type>/<slug>
    cd ../sim2d-<slug>
fi
```

## Branch Prefixes

| Task type | Branch prefix |
|-----------|---------------|
| Features  | `feat/`       |
| Fixes     | `fix/`        |
| Refactors | `refactor/`   |
| Docs      | `docs/`       |

## Plan Mode Requirement

When entering plan mode in a worktree, ALWAYS include at the top:

```markdown
## Context
- **Todo:** `docs/todo/<slug>.md`
- **Worktree:** `../sim2d-<slug>`
- **Branch:** `<type>/<slug>`
```

This persists context across conversation clears.

## Rules

1. Location: sibling dirs (`../sim2d-<slug>`)
2. Shared target dir via `~/.cargo/config.toml` — no cache copying needed
3. All work happens in the worktree, not the main repo
4. Never push — user pushes after review

## Examples

**Create new worktree:**

```bash
git worktree add ../sim2d-player-collision -b feat/player-collision
cd ../sim2d-player-collision
```

**Switch to existing worktree:**

```bash
cd ../sim2d-player-collision
```
