# Case 027: Wrong Algorithm for Problem

**Date:** 2026-01-24
**Session:** `47c22a9b-5674-4f78-ae66-2dd6c98bc867`
**Component:** Collision / Triangulation

## Context

Implementing terrain collision mesh generation.

## What Claude Did

Used an algorithm that produced no triangles for the collision mesh.

## User Response

*"The triangulation stage produces **no triangles** because we're using the wrong algorithm"*

## The Problem

Claude chose a triangulation algorithm that wasn't appropriate for the input data (polygons from marching squares). The algorithm ran without errors but produced empty output.

## Why This Happens

Algorithm selection requires understanding both:
1. What the algorithm does
2. What the input data looks like

Claude may select algorithms based on name/description without verifying they match the actual use case.

## Related

See also Case 010 (Non-Standard Algorithm Approach) for another algorithm mismatch in the same system.
