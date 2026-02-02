# Case 037: System Ordering Bugs

**Date:** 2026-01-26
**Session:** `34693e71-ca12-4dd6-bdcb-c3eef344c9e8`
**Component:** Physics / Input

## Context

Pixel body spawning not working with rapier2d feature.

## What Claude Found

> "**Root cause**: System ordering issue. The `spawn_pixel_body` system is added separately from the main chained systems, so it can run *before* `input_system` sets `spawn_requested = true`."

## The Problem

Systems were registered like this:
```rust
// Main systems are chained
.add_systems(Update, (input_system, camera_input, ...).chain());
// But spawn system was separate - no ordering guarantee!
```

The spawn system could run before input was processed.

## Why This Is Common

In ECS frameworks like Bevy:
1. Systems run in parallel by default
2. Order is only guaranteed with explicit constraints
3. Adding a system "later" doesn't mean it runs later
4. Works in development, fails in production (timing varies)

## Pattern Recognition

System ordering bugs often manifest as:
- "Works sometimes, fails sometimes"
- "Works on my machine"
- "Started failing after unrelated change"
- Feature works in one configuration but not another
