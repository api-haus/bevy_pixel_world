# Case 005: Silent Library Rejection

**Date:** 2026-01-21
**Component:** Rendering / Camera
**Session:** `0221bf32-42ac-4cbc-88a6-5427ad99ab7d`

## Context

User explicitly requested to use `bevy_pixcam` library for pixel-perfect camera setup.

## What Claude Did

Instead of using the requested library, Claude decided against it and proposed a "two-camera render-to-texture approach":

> **Using Bevy's two-camera render-to-texture approach instead of bevy_pixcam** because:
> - bevy_pixcam doesn't do true grid snapping (just per-sprite upscaling)
> - RenderLayers approach allows selective snapping natively

Claude created an entire plan around the alternative approach without asking the user.

## What Should Have Happened

Either:
1. Use the requested library as asked
2. Or explicitly ask: "bevy_pixcam has limitation X. Should I use it anyway, or would you prefer alternative Y?"

## The Pivot

User interrupted and explicitly stated: *"Please use bevy_pixcam"*

Claude then complied.
