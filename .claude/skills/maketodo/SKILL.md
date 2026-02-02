---
name: maketodo
description: Scan docs, populate docs/todo with tasks
---

# Make Todo

Read documentation and populate docs/todo with tasks.

## Process

1. **Read architectural docs** - Review `docs/architecture/` for design concepts
2. **Read implementation docs** - Check `docs/implementation/` for current state and plans
3. **Scan codebase** - Identify gaps between docs and implementation
4. **Update docs/todo/** - Create/update task files:
   - One file per task category (e.g., `pixel-world.md`, `modularity.md`, `performance.md`)
   - Each task: one-line description, optionally a brief context line
   - Mark tasks as `[ ]` (todo) or `[x]` (done)

## Task File Format

```markdown
# [Category] Tasks

## High Priority
- [ ] Brief task description
- [x] Completed task

## Low Priority
- [ ] Another task
```

## Constraints

- Do NOT implement tasks - only identify and document them
- Do NOT push - user reviews first
