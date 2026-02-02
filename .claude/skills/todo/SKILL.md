---
name: todo
description: Add tasks or ideas discovered during work to docs/todo
---

# Todo

Save tasks, ideas, or follow-ups discovered while working to docs/todo.

## Usage

`/todo <slug>: <description>` - Add a task with explicit slug
`/todo <description>` - Add to an existing category file

## Naming Convention

| Component | Format | Example |
|-----------|--------|---------|
| Todo file | `docs/todo/<slug>.md` | `docs/todo/player-collision.md` |
| Worktree | `../sim2d-<slug>` | `../sim2d-player-collision` |
| Branch | `<type>/<slug>` | `feat/player-collision` |

## Process

### New standalone task (with slug):
1. Create `docs/todo/<slug>.md` with task description
2. Format: single task per file for worktree association

### Addition to category file (no slug):
1. Determine the appropriate category file:
   - `player-integration.md` - Player collision, tools, camera
   - `refactoring.md` - Code quality, complexity reduction
   - `modularity.md` - Generic framework refactoring
   - `future-features.md` - New features, procedural generation, particles
   - `small-tasks.md` - Miscellaneous, doesn't fit elsewhere
2. Append the task as `- [ ] <description>`

## Task File Format

**Standalone task (for `/picktodo`):**
```markdown
# <Title>

<Description of what needs to be done>

## Tasks
- [ ] Subtask 1
- [ ] Subtask 2
```

**Category file:**
```markdown
# [Category] Tasks

## High Priority
- [ ] Brief task description

## Low Priority
- [ ] Another task
```

## Constraints

- Keep task descriptions brief
- Use kebab-case for slugs
- Do NOT implement - only record
