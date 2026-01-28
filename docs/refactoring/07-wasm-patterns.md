# WASM Patterns (Reference)

Documents current WASM compatibility patterns. No changes needed - this is reference documentation.

## Overview

The crate supports both native and WASM targets through conditional compilation. Key patterns are documented here for consistency when adding new platform-specific code.

## Noise Module Architecture

**Location:** `seeding/noise/`

```
seeding/noise/
├── mod.rs          # Re-exports unified Noise2d type
├── native.rs       # FastNoise2 wrapper (native only)
└── wasm.rs         # Pure Rust implementation (WASM)
```

### Pattern: Platform-Specific Implementations

```rust
// mod.rs
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::Noise2d;

#[cfg(target_arch = "wasm32")]
mod wasm;
#[cfg(target_arch = "wasm32")]
pub use wasm::Noise2d;
```

**Key Points:**
- Same public type name (`Noise2d`) on both platforms
- Same public API surface
- Implementation details hidden
- Consumer code doesn't need `#[cfg]` attributes

### API Compatibility

Both implementations provide:
```rust
impl Noise2d {
    pub fn new(seed: i32) -> Self;
    pub fn gen_2d(&self, x_range: Range<f32>, y_range: Range<f32>, ...) -> Vec<f32>;
    pub fn gen_single_2d(&self, x: f32, y: f32, seed: i32) -> f32;  // May be unused
}
```

## Conditional Compilation Patterns

### Project Rule: No Duplicate Definitions

From `CLAUDE.md`:
> Never duplicate functions, types, or entrypoints for `#[cfg]` gating.
> Apply `#[cfg]` to inner fields, statements, and scopes instead.

**Correct:**
```rust
pub struct Config {
    pub name: String,
    #[cfg(not(target_arch = "wasm32"))]
    pub thread_count: usize,
}
```

**Incorrect:**
```rust
#[cfg(not(target_arch = "wasm32"))]
pub struct Config {
    pub name: String,
    pub thread_count: usize,
}

#[cfg(target_arch = "wasm32")]
pub struct Config {
    pub name: String,
}
```

### Exception: Entirely Different Implementations

When native and WASM implementations share no code, separate modules are acceptable (as in `noise/`). The key is that the *public interface* remains unified.

## Time Compatibility

**Issue:** `std::time::Instant` is not available on WASM.

**Solution:** Use `web-time` crate or feature-gated time access:

```rust
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

#[cfg(target_arch = "wasm32")]
use web_time::Instant;
```

Or via a compatibility shim:
```rust
// time_compat.rs
#[cfg(not(target_arch = "wasm32"))]
pub use std::time::Instant;

#[cfg(target_arch = "wasm32")]
pub use web_time::Instant;
```

## File System Compatibility

**Native:** Direct filesystem access via `std::fs`
**WASM:** IndexedDB or in-memory storage

**Pattern:** Abstract behind `StorageBackend` trait (see `persistence/backend.rs`):
```rust
pub trait StorageBackend: Send + Sync {
    fn open(&self, path: &str) -> BoxFuture<'_, Result<Box<dyn StorageFile>>>;
    fn create(&self, path: &str) -> BoxFuture<'_, Result<Box<dyn StorageFile>>>;
    // ...
}
```

Implementations:
- `NativeBackend` - Uses `std::fs`
- `WasmBackend` - Uses IndexedDB (via `idb` crate or similar)

## Adding New Platform-Specific Code

Checklist:
1. Define a common trait or type interface
2. Implement separately in `native.rs` / `wasm.rs` if implementations differ significantly
3. Use inner `#[cfg]` for minor differences
4. Re-export unified type from `mod.rs`
5. Verify both targets compile:
   ```bash
   cargo build -p bevy_pixel_world
   cargo build -p bevy_pixel_world --target wasm32-unknown-unknown
   ```

## Current WASM Limitations

1. **No multi-threading** - `rayon` parallelism disabled on WASM
2. **No native filesystem** - Must use browser storage APIs
3. **Performance** - Generally slower than native, especially for noise generation
4. **No FastNoise2** - Pure Rust noise implementation used instead
