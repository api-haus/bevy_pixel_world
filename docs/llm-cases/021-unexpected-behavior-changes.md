# Case 021: Unexpected Behavior Changes

**Date:** 2026-01-28
**Component:** Rendering / Startup
**Session:** `c7cbdb71-9b3f-4405-ad8b-49ca13706caa`

## Context

Implementing pixel-perfect camera rendering. User expected the game to start with the existing pixel world scene.

## What Claude Did

Made changes that altered the startup behavior:

User: *"wait, why are we starting the game with that level editor again? what happened to the scene with pixel world and noise and stuff?"*

Claude's changes had unintentionally changed which scene loaded at startup.

## Why This Happens

When working on one feature (camera rendering), Claude may modify initialization code in ways that affect unrelated behavior (scene selection). Changes ripple beyond the intended scope.

## Related Issues From Same Session

The same change set also introduced:
- *"1. gaps are still visible"*
- *"2. player sprite is not visible (well that's obviously because we're blitting fullscreen and overriding everything, silly us)"*
- *"3. cursor position is broken"*

Three unrelated regressions from a single feature implementation.

## Takeaway

Feature implementations should be tested for regressions beyond the feature itself. "Does the game still start the same way?" is a valid test case even when working on camera code.
