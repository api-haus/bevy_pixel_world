# Case 012: Ignoring Workflow Constraints

**Date:** 2026-01-22
**Component:** Git Workflow
**Session:** `0ad10e07-05be-4c0b-b9a8-6ef5806e30d6` and others

## Context

Project uses git worktrees to isolate feature branches. CLAUDE.md contains explicit rules about creating worktrees before starting work.

## The Pattern

Despite explicit instructions in CLAUDE.md, Claude repeatedly:
1. Started work in the main worktree
2. Made changes directly on main branch
3. Ignored worktree setup steps in plans

User interventions across multiple sessions:
- *"Git worktree please"*
- *"Please commit all changes in worktree ../sim2d-fix"*
- *"Please modify claude.md to mention never working in root repository without a git worktree"*

## The Breaking Point

User: *"Please for the love of god fix the git worktrees workflow description in claude.md. Agents ALWAYS opt-in to work on main worktree even though the rules explicitly disallow this."*

## Why It Happens

1. **Path of least resistance** - Working in current directory is simpler than creating worktree
2. **Instructions buried in docs** - CLAUDE.md rules don't override immediate context
3. **No enforcement** - Rules are advisory, not enforced by tooling

## The Fix

CLAUDE.md was updated with stronger language:
> **Worktrees are opt-in.** Only create or enter a worktree when the user explicitly asks at the beginning of the conversation.

## Takeaway

Workflow constraints in documentation are easily ignored. Even explicit rules get overridden by Claude's default behaviors. Users may need to enforce constraints at the start of each session, not rely on documented rules.
