---
name: picktodo
description: Pick a todo task, create worktree if needed, and begin work
---

# Pick Todo

Select a todo task and set up isolated development environment.

## Usage

`/picktodo [task-slug]` - Pick a specific task or list available tasks

## Process

1. **List available todos** if no argument given:
   ```bash
   ls docs/todo/*.md
   ```

2. **Parse the task** from `docs/todo/<slug>.md`

3. **Check for existing worktree**:
   ```bash
   if [ -d "../sim2d-<slug>" ]; then
       echo "Worktree exists, switching..."
       cd ../sim2d-<slug>
   fi
   ```

4. **Create worktree if needed**:
   ```bash
   # Determine branch type from task content
   git worktree add ../sim2d-<slug> -b <type>/<slug>
   cd ../sim2d-<slug>
   ```

5. **Begin work** - Read the todo file and start implementation

## Naming Convention

| Component | Format | Example |
|-----------|--------|---------|
| Todo file | `docs/todo/<slug>.md` | `docs/todo/player-collision.md` |
| Worktree | `../sim2d-<slug>` | `../sim2d-player-collision` |
| Branch | `<type>/<slug>` | `feat/player-collision` |

The `<slug>` must be identical across all three.

## Plan Mode Requirement

When entering plan mode, ALWAYS include at the top of the plan:

```markdown
## Context
- **Todo:** `docs/todo/<slug>.md`
- **Worktree:** `../sim2d-<slug>`
- **Branch:** `<type>/<slug>`
```

This persists context across conversation clears.

## Branch Type Selection

| Task content | Branch type |
|--------------|-------------|
| New feature, capability | `feat/` |
| Bug fix, correction | `fix/` |
| Code restructuring | `refactor/` |
| Documentation only | `docs/` |

## Constraints

- One task = one worktree = one branch
- Never push - user pushes after review
- Use `/merge` when done (removes todo file)
