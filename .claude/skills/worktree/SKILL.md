---
name: worktree
description: Create or switch to a git worktree for isolated feature/fix development
---

Create a new git worktree or switch to an existing one. Only use when explicitly requested.

## Behavior

- If the worktree already exists → switch to it
- If the worktree doesn't exist → create it, then switch to it

## Usage

```bash
# Check if worktree exists
if [ -d "../sim2d-<name>" ]; then
    cd ../sim2d-<name>
else
    git worktree add ../sim2d-<name> -b <type>/<description>
    cd ../sim2d-<name>
fi
```

## Branch Prefixes

| Task type | Branch prefix |
|-----------|---------------|
| Features  | `feat/`       |
| Fixes     | `fix/`        |
| Refactors | `refactor/`   |
| Docs      | `docs/`       |

## Rules

1. Location: sibling dirs (`../sim2d-<descriptive-suffix>`)
2. Shared target dir via `~/.cargo/config.toml` — no cache copying needed
3. All work happens in the worktree, not the main repo
4. Never push — user pushes after review

## Examples

**Create new worktree:**

User: "Create a worktree for adding player movement"

```bash
git worktree add ../sim2d-player-movement -b feat/player-movement
cd ../sim2d-player-movement
```

**Switch to existing worktree:**

User: "Switch to the player-movement worktree"

```bash
cd ../sim2d-player-movement
```

Then continue working in that directory.
