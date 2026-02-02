---
name: maketodo
description: Scan docs, populate docs/todo with tasks
---

# Make Todo

Read documentation and populate docs/todo with tasks.

## Naming Convention

| Component | Format | Example |
|-----------|--------|---------|
| Todo file | `docs/todo/<slug>.md` | `docs/todo/player-collision.md` |
| Worktree | `../sim2d-<slug>` | `../sim2d-player-collision` |
| Branch | `<type>/<slug>` | `feat/player-collision` |

Use kebab-case slugs that match potential branch names.

## Process

1. **Read architectural docs** - Review `docs/architecture/` for design concepts
2. **Read implementation docs** - Check `docs/implementation/` for current state and plans
3. **Scan codebase** - Identify gaps between docs and implementation
4. **Create todo files** - One file per discrete task:
   - Filename: `docs/todo/<slug>.md`
   - Each task should be self-contained enough for a single worktree

## Task File Format

```markdown
# <Title>

<Brief description of what needs to be done>

## Tasks
- [ ] Subtask 1
- [ ] Subtask 2
- [ ] Subtask 3
```

## Constraints

- One task per file for clean worktree association
- Use kebab-case slugs (will become branch names)
- Do NOT implement tasks - only identify and document them
- Do NOT push - user reviews first
