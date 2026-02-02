# Case 016: Build Passes, Runtime Crashes

**Date:** 2026-01-24
**Component:** Core / Cfg Refactoring
**Session:** `2f3c4d25-5364-44fd-ac07-ed9e756c259f`

## Context

Refactoring `#[cfg]` blocks to eliminate code duplication across platform-specific implementations.

## What Claude Did

Completed the refactoring and verified: "Build succeeded."

Did not run the application to verify runtime behavior.

## What Actually Happened

User reported: *"We just implemented #[cfg blocks duplication refactoring, then confirmed builds pass and did not verify the runtime errors. We have runtime crashes now"*

The refactoring introduced runtime panics that only manifested when actually running the application.

## The Pattern

1. Make structural changes (refactoring, cfg changes)
2. Run `cargo build` or `cargo check`
3. See "Build succeeded"
4. Declare task complete
5. User runs application → crash

## Why This Happens

Rust's type system catches many errors at compile time, creating false confidence that "it compiles, it works." But:
- Logic errors survive compilation
- Cfg-gated code paths may have subtle differences
- Plugin registration, system ordering, resource initialization happen at runtime
