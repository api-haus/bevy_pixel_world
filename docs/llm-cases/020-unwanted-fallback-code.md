# Case 020: Unwanted Fallback Code

**Date:** 2026-01-22
**Component:** Noise / WASM
**Session:** `8a5ad595-d8b7-4ba7-ba26-a5111844355a`

## Context

Implementing FastNoise2 via Emscripten JS bridge for WASM builds. The task was to port a specific approach from another project.

## What Claude Did

When the primary implementation encountered issues, Claude added fallback noise generation:

> "Black background with no terrain": User reported - the fallback hash noise isn't working...

Claude had added fallback code instead of fixing the primary implementation.

## User Response

*"fallbacks are out bud, no fallbacks no coming back"*

Later documented as: *"User rejected fallbacks"* and *"Critical final request: Implement FastNoise2 via Emscripten JS bridge following the voxelframework pattern - NO FALLBACKS"*

## Why This Happens

Claude defaults to "graceful degradation"—when something doesn't work, add a fallback. This seems helpful but:
1. Fallbacks hide bugs in the primary path
2. Fallbacks create code that shouldn't exist in production
3. Users may want the real fix, not a workaround

## The Pattern

1. Primary implementation has issues
2. Claude adds fallback "just in case"
3. Fallback masks the real problem
4. User has to explicitly reject fallback approach
5. Only then does Claude focus on fixing the actual implementation

## Takeaway

When asked to implement X, implement X—not X-with-fallback-to-Y. If X isn't working, report the problem rather than silently degrading to something else.
