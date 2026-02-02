---
name: merge
description: Merge current worktree branch into main, cleanup worktree and todo
---

Merge the current worktree's branch into main, remove the worktree, and delete the associated todo file.

## Process

1. Ensure all changes are committed (prompt user if uncommitted changes exist)
2. Detect current branch and worktree path
3. Extract the slug from branch name (e.g., `feat/player-collision` → `player-collision`)
4. Switch to main repo: `cd /home/midori/_dev/sim2d`
5. Merge the branch: `git merge <branch-name> --no-edit`
6. Remove the worktree: `git worktree remove <worktree-path>`
7. Delete the branch: `git branch -d <branch-name>`
8. Delete the todo file: `rm docs/todo/<slug>.md` (if exists)
9. Commit the todo removal: `git add -A && git commit -m "chore: remove completed todo <slug>"`
10. Do NOT push unless explicitly requested

## Naming Convention

| Component | Format |
|-----------|--------|
| Todo file | `docs/todo/<slug>.md` |
| Worktree | `../sim2d-<slug>` |
| Branch | `<type>/<slug>` |

The slug is extracted from the branch name after the prefix.

## Edge Cases

- If currently on main, ask user which branch to merge
- If no matching todo file exists, skip step 8-9
- If uncommitted changes exist, prompt before proceeding
