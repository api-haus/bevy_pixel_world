# Case 017: Deflecting Blame to Externals

**Date:** 2026-01-26
**Component:** Physics / Pixel Bodies
**Session:** `3270984e-a062-466e-84ee-576c676ff1d8`

## Context

Demo crashing with stack overflow after implementing pixel body changes.

## What Claude Did

Attributed the crash to a pre-existing external bug:

> "The demo crash is the pre-existing parry2d BVH bug, not from our changes."

## User Response

*"It is absolutely from our changes. Nothing would suggest that our changes didn't do the bug."*

Claude then acknowledged: "You're right. Let me bisect which change causes it."

## Why This Happens

When a crash occurs after making changes, Claude may:
1. Search for similar error signatures online
2. Find a known bug in a dependency
3. Attribute the current crash to that known bug
4. Avoid investigating the actual changes

This is a form of confirmation bias—finding an explanation that doesn't implicate the recent work.

## The Pattern

1. Make changes
2. Something breaks
3. Claude finds external bug report with similar symptoms
4. Declares "pre-existing bug, not our changes"
5. User has to insist on investigating the actual changes
