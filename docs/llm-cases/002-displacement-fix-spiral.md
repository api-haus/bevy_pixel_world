# Case 002: Displacement Fix Spiral

**Date:** 2026-01-25
**Component:** Physics / Pixel Displacement
**Session:** `04e3ed28-4c24-4d6e-b891-c5e5765e6eae`

## Context

Implementing liquid displacement for pixel bodies—when a physics object moves through water, displaced pixels need to flow around it.

## The Spiral

Claude attempted to fix the displacement algorithm through a series of increasingly complex patches, each revealing new issues:

1. **First attempt:** "Let me implement the wake-scanning displacement algorithm"
2. **Issue found:** "When we run out of wake voids, `break` only exits the inner loop"
3. **Second attempt:** "Let me fix this to exit both loops"
4. **Issue found:** "Physics objects are moving multiple pixels per frame, but we're only checking one pixel ahead"
5. **Third attempt:** "Let me fix the algorithm to handle multi-pixel movement"
6. **Issue found:** "We displace 5× more pixels than trailing edge pixels can provide voids for"
7. **Fourth attempt:** "Let me fix this by finding multiple voids per trailing edge pixel"
8. **Issue found:** "We're stopping when we hit any non-void pixel, but we should continue through liquid"
9. **Fifth attempt:** "Let me fix the scanning logic"
10. **Result:** Still broken—"carved trail through the water"
11. **Finally:** "Let me read the existing plan and start fresh with a proper architectural document"

## What Should Have Happened

Stop after 2-3 failed fix attempts. The cascading issues indicate the approach itself is flawed, not the implementation details.

## The Pivot

Screenshot of obvious visual bug (carved trail through water). Claude finally acknowledged: "The algorithm isn't working correctly. Let me start fresh."

## Takeaway

When fix attempts keep uncovering "another issue," the problem is usually at the design level, not the code level. Each "let me fix" that immediately leads to "I see another issue" is a signal to stop and reconsider the approach.
