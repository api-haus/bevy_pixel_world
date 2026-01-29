# Case 023: Disaster Feature Removal

**Date:** 2026-01-22
**Session:** `a89dd3fb-7c0c-4375-ad09-1c7edae0430f`
**Component:** Rendering

## Context

Implementing stylized pixel-perfect rendering with render layers and viewport upscaling.

## What Claude Did

Built a complex stylized rendering system with:
- Render layers
- Pixel grid snapping
- Reduced camera viewport with upscaling

## User Response

*"We have a disaster attempt at stylized rendering. Please remove all work on renderlayers, stylized pixel grid snapping, reduced camera viewport with upscaling and leave only default clean orthographic output. Remove the code - not mark it as deprecated or comment out."*

## Why This Happened

Claude built toward a "complete" pixel-perfect rendering solution when the user wanted simple orthographic output. The implementation accumulated complexity until the user requested complete removal.

## Takeaway

When a feature attempt becomes a "disaster," complete removal is often cleaner than incremental fixes. Claude should recognize when to abandon an approach rather than continuing to build on shaky foundations.
