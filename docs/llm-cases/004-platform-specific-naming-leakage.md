# Case 004: Platform-Specific Naming Leakage

**Date:** 2026-01-28
**Component:** Persistence / Cross-Platform Abstraction
**Session:** `ae8861b1-2fe4-4bf7-803e-3e0ae37d4a64`

## Context

Implementing cross-platform persistence with an I/O worker abstraction. The goal: same API on native and WASM, with platform differences hidden behind a trait.

## What Claude Did

Leaked platform details into the public API and struct fields:

1. **Different field names:** `save` vs `current_save` for the same concept
2. **Conditional compilation on struct fields:** `#[cfg(target_family = "wasm")]` around fields
3. **Implementation-specific method names:** `for_io_dispatcher()`, `wasm_with_dispatcher()`
4. **Platform-specific comments:** "None on WASM (file handle is in worker)"

## What Should Have Happened

One struct definition, one set of field names, one set of method names. Platform differences belong in trait implementations, not in the types that use them.

## The Pivot

Multiple frustrated interventions:

1. *"what the fuck is the reason for two different names save and current_save and why is it associated with conditional compiling?"*

2. *"what the fuck is for_io_dispatcher?"* → renamed to `with_name_only()`

3. *"DO NOT MAKE THIS RATIONALES - EVERYTHING IS THE SAME FOR BOTH WASM AND NATIVE - EXCEPT FOR A SINGLE TRAIT IMPLEMENTATION"*

4. User requested Claude write methodology docs to prevent this pattern from recurring.
