# Module Visibility Cleanup

Reduce public API surface to clarify what's intended for external use.

## Current State

### `lib.rs` Re-exports

The crate re-exports extensively from internal modules. Current public surface includes:

**From `persistence`:**
- `BasicPersistencePlugin`, `PixelBodyRecord`, `WorldSave`, `WorldSaveResource`

**From `simulation`:**
- `HeatConfig`, `simulate_tick`
- Submodules `burning`, `hash`, `heat` are `pub mod`

**From `pixel_body`:**
- 15+ types: `Bomb`, `DisplacementState`, `LastBlitTransform`, `NeedsColliderRegen`, etc.
- Submodule `bomb` is `pub mod`

**From `rendering`:**
- `ChunkMaterial`, `Rgba`, texture functions

## Issues

1. **Submodules exposed unnecessarily** - `simulation::burning`, `simulation::hash`, `pixel_body::bomb` are public but likely internal implementation details

2. **Large re-export lists** - `lib.rs` lines 55-59 export 14 items from `pixel_body`

3. **Unclear API boundary** - No distinction between "stable public API" and "implementation details that happen to be public"

## Proposed Changes

### 1. Make Internal Submodules Private

**`simulation/mod.rs`**
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

**`pixel_body/mod.rs`**
```rust
// Before
pub mod bomb;

// After
mod bomb;

pub use bomb::{Bomb, BombShellMask};  // Only public types
// Internal functions like compute_bomb_shell stay private
```

### 2. Audit and Reduce Re-exports

Review each re-export in `lib.rs`. For each item ask:
- Is this used by external crates?
- Is this used by examples?
- Could this be accessed via a parent module instead?

**Candidates for removal from public API:**

| Item | Likely Internal? | Reason |
|------|------------------|--------|
| `LastBlitTransform` | Yes | Implementation detail of pixel body sync |
| `NeedsColliderRegen` | Yes | Internal marker component |
| `ShapeMaskModified` | Yes | Internal marker component |
| `simulate_tick` | Maybe | Users typically use plugin, not manual calls |

### 3. Document Public API

Add module-level documentation distinguishing:
```rust
//! # Public API
//!
//! ## Core Types
//! - [`PixelWorld`] - Main world container
//! - [`Pixel`] - Individual pixel data
//!
//! ## Plugins
//! - [`PixelWorldPlugin`] - Main Bevy plugin
//!
//! ## Configuration
//! - [`HeatConfig`] - Heat simulation parameters
```

## Verification Strategy

1. Make changes incrementally, one module at a time
2. After each change:
   ```bash
   cargo build -p game
   cargo build --examples
   cargo doc -p game
   ```
3. Fix any breakage before proceeding

## Verification Commands

```bash
cargo clippy -p game -- -D warnings
cargo build -p game
cargo build --examples
cargo test -p game
cargo doc -p game --no-deps
```

## Estimated Impact

- **Risk:** Medium - may break downstream code if visibility is reduced too aggressively
- **Lines changed:** ~20
- **Benefit:** Clearer API, easier maintenance, better documentation
- **Mitigation:** Make changes incrementally, test examples after each change
