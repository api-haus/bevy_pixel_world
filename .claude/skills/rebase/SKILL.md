---
name: rebase
description: Rebase current worktree branch onto latest main
---

Rebase the current worktree's branch onto the latest main branch.

## Pre-flight checks

1. Run `git status` to check for uncommitted changes
2. If uncommitted changes exist:
   - Ask user whether to commit, stash, or abort
   - Do NOT proceed until working tree is clean
3. Fetch latest from main worktree (no network fetch needed - it's local)

## Rebase

1. Get current branch name: `git branch --show-current`
2. Rebase onto main: `git rebase main`
3. If conflicts occur:
   - Show conflicted files: `git status`
   - Do NOT auto-resolve - ask user how to proceed
   - Options: fix manually, `git rebase --abort`, or `git rebase --skip`

## Post-rebase

1. Run `git status` to confirm clean state
2. Show new commit position: `git log --oneline -3`

## Notes

- This rebases from the LOCAL main branch (same repo, different worktree)
- No `git fetch` needed since worktrees share the same .git
- Never use `--no-edit` with rebase (it's not a valid option)
- Never force-push without explicit user request
