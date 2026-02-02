# Module Visibility Cleanup

Reduce public API surface to clarify what's intended for external use.

## Overview

The crate currently exposes too many internal implementation details through public modules and extensive re-exports. This task cleans up visibility to create a clearer API boundary.

## Tasks
- [ ] Make simulation submodules private (burning, hash, heat)
- [ ] Make pixel_body::bomb submodule private
- [ ] Audit and reduce re-exports in lib.rs
- [ ] Remove internal marker components from public API
- [ ] Add module-level documentation for public API
- [ ] Test examples after each visibility change

## Changes Required

### 1. Make Internal Submodules Private
**simulation/mod.rs:**
```rust
// Before
pub mod burning;
pub mod hash;
pub mod heat;

// After
mod burning;
mod hash;
mod heat;

pub use heat::HeatConfig;  // Only export what's needed
```

**pixel_body/mod.rs:**
```rust
// Before
pub mod bomb;

// After
mod bomb;

pub use bomb::{Bomb, BombShellMask};  // Only public types
```

### 2. Audit lib.rs Re-exports
Candidates for removal from public API:
| Item | Action |
|------|--------|
| `LastBlitTransform` | Make internal |
| `NeedsColliderRegen` | Make internal |
| `ShapeMaskModified` | Make internal |
| `simulate_tick` | Review if needed publicly |

### 3. Document Public API
Add to lib.rs:
```rust
//! # Public API
//!
//! ## Core Types
//! - [`PixelWorld`] - Main world container
//! - [`Pixel`] - Individual pixel data
//!
//! ## Plugins
//! - [`PixelWorldPlugin`] - Main Bevy plugin
```

## Verification Strategy

Make changes incrementally, one module at a time:

```bash
# After each change:
cargo build -p bevy_pixel_world
cargo build -p game  # Verify game still compiles
cargo doc -p bevy_pixel_world --no-deps
```

## Verification Commands

```bash
cargo clippy -p bevy_pixel_world -- -D warnings
cargo build -p bevy_pixel_world
cargo build -p game
cargo test -p bevy_pixel_world
cargo doc -p bevy_pixel_world --no-deps
```

## References
- docs/refactoring/05-module-visibility.md
