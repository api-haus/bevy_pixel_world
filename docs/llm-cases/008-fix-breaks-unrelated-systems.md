# Case 008: Fix Breaks Unrelated Systems

**Date:** 2026-01-28
**Component:** Rendering / Camera
**Session:** `a2ea63e0-1ec2-478d-a5ca-37d93825c489`

## Context

Fixing visible gaps between chunk tiles in pixel-perfect rendering.

## What Claude Did

Applied a "half-pixel overlap fix" that:
1. Changed mesh origin from center to bottom-left
2. Modified chunk positioning to compensate
3. Attempted texture overlap via UV manipulation

## What Actually Happened

User reported: *"gaps between chunks are even more visible now, and egui menus are non-functional because cursor positioning with egui is broken now for some reason"*

The mesh origin change that was supposed to fix gaps instead:
- Made gaps more visible (wrong fix)
- Broke egui cursor positioning (unrelated system)

## The Problem

Changing a fundamental assumption (mesh origin point) has cascading effects on systems that depended on that assumption. Egui's cursor-to-world positioning relied on the original coordinate system.

## The Pivot

Claude reverted: "The overlap approach made things worse - stretching the mesh while keeping UVs at [0,1] scales the texture rather than creating proper overlap."
