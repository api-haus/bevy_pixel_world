# Clippy Lint Fixes

Mechanical fixes for all current Clippy warnings.

## Tasks
- [ ] Fix `io::Error::other()` usage in persistence/backend.rs:57
- [ ] Fix 5 locations of `io::Error::other()` in persistence/native.rs (lines 36, 51, 66, 78, 92)
- [ ] Add `is_empty()` method to `StorageFile` trait in persistence/backend.rs
- [ ] Add explicit `truncate(false)` in persistence/native.rs:154-158
- [ ] Simplify dereference in debug_shim.rs:40
- [ ] Check and fix dead code `gen_single_2d` in seeding/noise/native.rs and seeding/noise/wasm.rs

## Changes Required

### 1. Use `io::Error::other()`
```rust
// Before
io::Error::new(io::ErrorKind::Other, e)

// After
io::Error::other(e)
```

### 2. Add `is_empty()` to trait
```rust
pub trait StorageFile: Send + Sync {
    fn len(&self) -> BoxFuture<'_, Result<u64, BackendError>>;
    
    fn is_empty(&self) -> BoxFuture<'_, Result<bool, BackendError>> {
        Box::pin(async { Ok(self.len().await? == 0) })
    }
}
```

### 3. Explicit truncate
```rust
let file = fs::File::options()
    .read(true)
    .write(true)
    .create(true)
    .truncate(false)  // Add this
    .open(path)?;
```

## Verification

```bash
cargo clippy -p game -- -D warnings
cargo build -p game
cargo test -p game
```

## References
- docs/refactoring/01-clippy-lint-fixes.md
