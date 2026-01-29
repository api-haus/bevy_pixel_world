# Case 025: Confusing Naming Conventions

**Date:** 2026-01-29
**Session:** `ae8861b1-2fe4-4bf7-803e-3e0ae37d4a64`
**Component:** Persistence

## Context

Persistence system with save file handling.

## What Claude Did

Created two similar names with different meanings tied to conditional compilation:
- `save`
- `current_save`

With `#[cfg]` attributes affecting which one was used.

## User Response

*"what the fuck is the reason for two different names save and current_save and why is it associated with conditional compiling?"*

## The Problem

1. Two names for related concepts creates confusion
2. The difference between them wasn't clear
3. Conditional compilation made it worse—the names changed meaning based on platform

## Why This Happens

During iterative development, names accumulate. `save` gets added, then `current_save` gets added for a slightly different purpose. Platform-specific code adds more divergence. Eventually the naming is a mess.

## Takeaway

Names should be clear without requiring context about conditional compilation. If two names are confusingly similar, unify them or make their distinction obvious.
