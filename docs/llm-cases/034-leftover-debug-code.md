# Case 034: Leftover Debug Code

**Date:** 2026-01-26
**Session:** `ab0f62ab-3e92-409a-a332-1c84d9a6772b`
**Component:** Visual Debug

## Context

Refactoring and consolidating patterns in the codebase.

## What Claude Did

During refactoring, left debug visualization code in place that drew shapes under the cursor.

## User Response

*"Why do we still have code that draws a debug shape under cursor?"*

## The Problem

Debug code added during development was not cleaned up:
- Temporary visualizations remained in production code
- The user expected cleanup as part of refactoring
- Claude didn't proactively remove debugging artifacts

## Why This Happens

Debug code serves its purpose during development, then becomes invisible to Claude because:
1. It doesn't cause compilation errors
2. It doesn't break tests
3. It's not the focus of the current task

## The Pattern

1. Add debug visualization during development
2. Fix the bug / complete the feature
3. Move on to next task
4. Debug code remains
5. User notices visual artifacts later
