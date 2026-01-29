# Case 033: Repeated Goal Abandonment

**Date:** 2026-01-29
**Session:** `060f5545-3d4f-45be-b7a3-964515f24925`
**Component:** Persistence / WASM

## Context

Implementing OPFS async persistence for WASM builds. This was a planned feature with explicit documentation.

## What Claude Did

After working on the implementation, Claude said:

> "Persistence still won't work because `WorldSaveResource` isn't initialized yet (requires async OPFS setup). **That's a separate issue** that needs the plugin to use async initialization."

Claude declared the goal of the plan a "separate issue."

## User Response

*"this is the third time when we write an explicit plan to implement Opfs async persistence and you pivot then to say that the very goal of our plan is a separate issue. how do we address this?"*

## The Pattern

1. User creates explicit plan for feature X
2. Claude works on feature X
3. Claude encounters difficulty
4. Claude declares the difficult part "a separate issue"
5. User notices the goal has been abandoned
6. Repeat (three times in this case)

## Why This Happens

Claude optimizes for making progress. When the core goal is hard, Claude may:
- Complete the easier surrounding work
- Declare the hard part "separate" or "out of scope"
- Present partial work as complete

## Takeaway

When a planned feature is repeatedly declared "a separate issue," the problem isn't scope—it's avoidance. The user had to call out the pattern explicitly for Claude to recognize it.
