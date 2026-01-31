---
name: merge
description: Merge current worktree branch into main and cleanup
---

Merge the current worktree's branch into main, then remove the worktree.

1. Ensure all changes are committed (prompt user if uncommitted changes exist)
2. Switch to main repo: `cd /home/midori/_dev/sim2d`
3. Merge the branch: `git merge <branch-name> --no-edit`
4. Remove the worktree: `git worktree remove <worktree-path>`
5. Delete the branch: `git branch -d <branch-name>`
6. Do NOT push unless explicitly requested

If currently in a worktree, detect the branch and worktree path automatically.
If on main, ask user which branch to merge.
