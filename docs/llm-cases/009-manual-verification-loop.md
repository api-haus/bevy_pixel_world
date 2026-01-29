# Case 009: Manual Verification Loop

**Date:** 2026-01-26
**Component:** Testing / Pixel Bodies
**Session:** `3270984e-a062-466e-84ee-576c676ff1d8`

## Context

Debugging a crash (stack overflow) when spawning pixel bodies and scrolling around to load/unload chunks.

## What Claude Did

Repeatedly launched the demo example to manually verify behavior:
- Run demo
- Observe crash
- Make change
- Run demo again
- Repeat

This continued through multiple iterations without establishing automated reproduction.

## The Intervention

User explicitly requested: *"Would you please stop running the demo? Establish an automated e2e test."*

## Why This Matters

Manual verification:
- Doesn't persist as regression protection
- Can't be run in CI
- Requires human to observe results
- Wastes time re-verifying the same scenario

An automated E2E test would:
- Reproduce the exact crash scenario
- Run without human observation
- Catch regressions automatically
- Document the failure condition in code

## Takeaway

When debugging complex scenarios, Claude defaults to manual "run and observe" verification. Users may need to explicitly redirect toward writing automated tests that capture the failure condition.
