# Case 022: API Scope Over-Design

**Date:** 2026-01-26
**Component:** Persistence / Public API
**Session:** `5999f64a-7e28-4da7-8f88-20885c2e0f2d`

## Context

Designing the public persistence API for end consumers.

## What Claude Proposed

Claude was building toward a comprehensive persistence API with features like:
- `list_saves()` - enumerate available saves
- Directory management for save files
- Save file discovery

## User Constraint

*"our system does not need anything like listing save files, or latching onto a directory of saves, it must work with absolute file system paths. we only need to be able to save, load and copy-on-write"*

## The Pattern

When designing APIs, Claude tends toward "complete" solutions:
- Every persistence system "should" have save listing
- Every file system API "should" handle directory discovery
- Every storage layer "should" manage multiple saves

But users often want minimal APIs that do one thing well.

## Why This Happens

Claude models "good API design" based on patterns from large systems. But smaller projects benefit from simpler APIs that:
1. Do fewer things
2. Push complexity to the consumer when appropriate
3. Don't anticipate features the user hasn't requested
