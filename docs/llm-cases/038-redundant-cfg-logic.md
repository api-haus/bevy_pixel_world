# Case 038: Redundant Cfg Logic

**Date:** 2026-02-02

## The Code

```rust
#[cfg(any(
  feature = "avian2d",
  all(feature = "rapier2d", not(feature = "avian2d"))
))]
```

## What It Means

- avian2d, OR
- rapier2d AND NOT avian2d

## What It Simplifies To

```rust
#[cfg(any(feature = "avian2d", feature = "rapier2d"))]
```

## Why This Is Special

The second branch explicitly excludes avian2d, but the first branch already handles avian2d. The nested `all(..., not(...))` adds nothing—it's boolean theater.
