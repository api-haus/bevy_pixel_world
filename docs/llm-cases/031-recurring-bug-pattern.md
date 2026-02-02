# Case 031: Recurring Bug Pattern

**Date:** 2026-01-21
**Session:** `428ade98-6bfa-4963-951b-ea5b3e3d0325`
**Component:** Input / Flight

## Context

Flight mechanic where holding spacebar should allow continuous flight.

## What Claude Did

Implemented flight using `On<Fire<Fly>>` observer pattern.

## User Response

*"Holding spacebar to fly not working again. Please fix."*

The word "again" indicates this bug had appeared before and been "fixed" previously.

## The Problem

Claude acknowledged: "The issue is that `On<Fire<Fly>>` only triggers once when the action fires, not continuously while held."

The fix changed from observer pattern to polling the action state.

## Why This Happens

1. Initial implementation works
2. Refactoring or "improvement" breaks it
3. User reports it's broken
4. Claude fixes it
5. Later change breaks it again
6. "Again" indicates the cycle

## Pattern Indicator

The word **"again"** in bug reports signals:
- The fix didn't address root cause
- A regression was introduced
- The system is fragile to changes
