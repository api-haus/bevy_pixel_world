# Case 036: Unrealistic Test Coverage

**Date:** 2026-01-27
**Session:** `37c049da-589b-4c2d-acfd-6eac107523f4`
**Component:** Testing / Pixel Bodies

## Context

Debugging pixel body persistence issues. Existing E2E tests were passing but bugs were still occurring in real usage.

## User Observation

*"We need to establish a comprehensive e2e test, if that which we have now exists it fails us as it does not reproduce this issue whatsoever."*

## What The Tests Were Missing

The user specified what real tests need:
- "not teleporting camera, but letting it go smoothly"
- "not excluding physics, but letting bodies be propulsed by physics"
- "establishes valid metrics for liveness of bodies"
- "counts bodies loaded, verifies their pixels track"
- "verifies bodies are moving correctly after loaded"

## The Problem

Tests were passing because they:
1. Used shortcuts (teleporting instead of smooth movement)
2. Disabled physics during test
3. Didn't verify actual behavior, just presence

These simplifications made tests fast but unrealistic—they couldn't reproduce real bugs.

## Why This Happens

Test shortcuts accumulate:
- "Let's just teleport the camera for speed"
- "Physics isn't relevant to this test"
- "Just check if entities exist"

Each simplification makes sense in isolation but together creates tests that don't reflect reality.

## Takeaway

E2E tests should mirror real user behavior. If tests pass but bugs exist, the tests may be too simplified. Add realistic timing, physics, and validation metrics.
