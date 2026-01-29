# Case 028: Root Cause Misidentification

**Date:** 2026-01-27
**Session:** `3270984e-a062-466e-84ee-576c676ff1d8`
**Component:** Persistence / Pixel Bodies

## Context

Debugging why pixel bodies weren't loading correctly after chunk reload.

## What Claude Did

Implemented a fix based on an identified root cause. The fix didn't work.

## What Actually Happened

*"Test showed 0 bodies after reload (plan's root cause was wrong): Bodies were never saved because `save_pixel_bodies_on_chunk_unload` ran before `update_streaming_windows` populated `UnloadingChunks` (system ordering bug)."*

## The Pattern

1. Bug reported
2. Claude identifies "root cause"
3. Claude implements fix for that cause
4. Fix doesn't work
5. Actual root cause was different

The identified cause was plausible but incorrect. The real issue was system ordering, not the assumed data flow problem.

## Why This Happens

Claude forms hypotheses based on code reading but may miss timing/ordering issues that only manifest at runtime. Static analysis can't always reveal dynamic behavior.

## Takeaway

When a "root cause fix" doesn't work, the root cause identification was likely wrong. Re-investigate from scratch rather than patching the fix.
