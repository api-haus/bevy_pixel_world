# Case 010: Non-Standard Algorithm Approach

**Date:** 2026-01-25
**Component:** Collision / Marching Squares
**Session:** `47c22a9b-5674-4f78-ae66-2dd6c98bc867`

## Context

Implementing marching squares for collision mesh generation. The implementation was producing fragmented polylines instead of clean closed contours.

## What Claude Did

Implemented a two-phase approach:
1. Extract edge segments from each cell
2. `connect_segments()` to chain segments into closed polylines

Claude diagnosed the bug as "floating-point epsilon comparison issues" in segment connection.

## The User's Question

*"It doesn't sound like a typical problem one encounters with writing a marching squares algorithm, why are we connecting segments?"*

## Claude's Realization

> "You're right to question this. In a **typical** marching squares implementation, you don't extract segments and connect them afterward. Instead, you **trace contours directly**."

The standard approach traces contours as they're found, following edges cell-by-cell, naturally producing closed loops without needing a separate connection phase.

## The Problem

Claude's approach created an unnecessary intermediate step (disconnected segments) that then required complex reconnection logic with floating-point tolerance issues. The standard algorithm avoids this entirely by never disconnecting the contour in the first place.

## Takeaway

When implementing well-known algorithms, Claude may invent novel approaches that create problems the standard approach avoids. Users questioning "why are we doing X?" can reveal when Claude has strayed from established solutions.
