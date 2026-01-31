---
name: worktree
description: Create a git worktree for isolated feature/fix development
---

Create a new git worktree for isolated development. Only use when explicitly requested.

## Usage

```bash
git worktree add ../sim2d-<name> -b <type>/<description>
cd ../sim2d-<name>
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

## Example

User asks: "Create a worktree for adding player movement"

```bash
git worktree add ../sim2d-player-movement -b feat/player-movement
cd ../sim2d-player-movement
```

Then continue working in that directory.
