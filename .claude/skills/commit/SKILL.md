---
name: commit
description: Commit all changes including untracked files
---

Commit all changes in the current working directory (or worktree).

1. Run `git status` and `git diff --stat` to see changes
2. Stage ALL files including untracked: `git add -A`
3. Write a commit message following the repo's conventional commit style:
   - `feat:` for new features
   - `fix:` for bug fixes
   - `docs:` for documentation
   - `refactor:` for refactoring
   - `build:` for build system changes
4. Include `Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>`
5. Do NOT push unless explicitly requested
