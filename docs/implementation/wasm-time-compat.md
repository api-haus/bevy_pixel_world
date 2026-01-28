# WASM Time Compatibility

## The Problem

`std::time::Instant` panics on `wasm32-unknown-unknown` because browsers don't provide the monotonic clock API that `Instant` expects. This causes runtime crashes when code using `Instant::now()` runs in the browser.

## The Solution

Use the `web-time` crate, which provides a drop-in replacement that works on both native and WASM:

```rust
// WASM compat: std::time::Instant panics on wasm32
use web_time::Instant;
```

## Rules

1. **Never use `std::time::Instant`** in library code that may run on WASM
2. **Always use `web_time::Instant`** instead
3. **`std::time::Duration` is fine** - it's just a data type, no platform APIs
4. **`std::time::SystemTime`** also needs WASM handling (use `js_sys::Date::now()`)

## Cargo.toml Pattern

Add a comment above the dependency to document why it exists:

```toml
# WASM compat: use web_time::Instant, NOT std::time::Instant
web-time = "1.1"
```

## Source File Pattern

Add a comment explaining the substitution:

```rust
use std::time::Duration;  // Duration is fine

// WASM compat: std::time::Instant panics on wasm32
use web_time::Instant;
```

## Exceptions

Test files that only run on native (in `tests/` directories or `#[cfg(test)]` modules) can use `std::time::Instant` directly since they never run on WASM.

## Quick Check

```bash
# Find potential violations
rg "std::time::Instant" --type rust -g '!tests/*' -g '!*_test.rs'
```
