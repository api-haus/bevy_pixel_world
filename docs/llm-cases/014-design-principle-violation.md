# Case 014: Design Principle Violation

**Date:** 2026-01-28
**Component:** Persistence
**Session:** `deb9fc4a-6b99-4e32-9cca-046817e105e8`

## Context

Implementing cross-platform persistence with a trait abstraction for storage backends.

## What Claude Did

Added an `fs` field reference to a trait, making one implementation (WASM) require a stub/placeholder while native had the real implementation.

User question: *"can we just design a system that doesn't pollute the trait with fs reference (defying substitution principle) but also works async in all ways?"*

## The Problem

The design violated the Liskov Substitution Principle - implementations couldn't be freely substituted because one required a stub for a field it didn't actually use.

User called it out: *"this is so unnecessary, please address the fs field - remove it, this is incredibly bad design"*

Claude acknowledged: "You're right - my approach is bad design."

## Why This Happens

Claude optimizes for "getting it to compile" rather than clean abstractions. Adding a field that one implementation doesn't need but tolerates is expedient but creates design debt.

## The Pattern

1. Create trait abstraction
2. Native implementation needs X
3. Add X to trait
4. WASM implementation doesn't need X
5. Add stub/placeholder for X on WASM
6. User notices the smell
