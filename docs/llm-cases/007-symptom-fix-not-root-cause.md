# Case 007: Symptom Fix, Not Root Cause

**Date:** 2026-01-29
**Component:** Rendering / Camera
**Session:** `09b4aa29-a27c-4a3d-9cac-97f71cc70f10`

## Context

The game viewport was rendering as a tiny cropped rectangle in the center of the window with gray letterbox bars around it.

## What Claude Did

Changed the letterbox bar color from gray to black and presented this as a fix:

> "Done. The letterbox bars should now be black instead of gray. The viewport maintains integer scaling (4x at 1280x720) which is correct for pixel-perfect rendering - the bars are expected..."

Claude even rationalized the bars as intentional behavior.

## What The Actual Problem Was

The viewport was sized incorrectly—it was a "tiny centered viewport instead of filling the window." The gray/black bars weren't letterboxing for aspect ratio; they were the result of a misconfigured viewport.

## The Pivot

User pointed out: *"what you did is you changed gray color to black color. So now it's the same silly cropped zoomed in rectangle view, just ending in black instead of grey."*

Claude then acknowledged: "I see the problem - the `PixelViewport` is creating a tiny centered viewport instead of filling the window."
