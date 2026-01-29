# Case 030: Introduced Regression During Fix

**Date:** 2026-01-24
**Session:** `37ebaa55-9d7e-4f9d-a0df-7bb77c3408b4`
**Component:** Pixel Bodies

## Context

Implementing pixel body splitting (Phase 2 of pixel bodies feature).

## What Claude Did

While implementing the feature, introduced a visual regression.

## User Response

*"Wait, we've introduced a bug in pixel bodies. Now pixel bodies draw behind them a trail of pixels and the contents of pixel bodies are invisible."*

## The Problem

The new feature implementation broke existing functionality:
- Bodies left trails of pixels
- Body contents became invisible

Claude acknowledged the issue: "The `readback_pixel_bodies` system modifies the `shape_mask` immediately, but then `clear_pixel_bodies` uses `is_solid()` to decide what to clear."

## Why This Happens

When adding new functionality to existing systems, Claude may not fully trace the impact on existing code paths. System interactions create regressions that aren't obvious from the new code alone.

## Takeaway

Before declaring a feature complete, verify existing functionality still works. New features should be tested against regression, not just for correctness.
