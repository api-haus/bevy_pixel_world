# Clippy Lint Fixes

Quick wins - mechanical fixes for all current Clippy warnings.

## Issues

### 1. Use `io::Error::other()` instead of `io::Error::new(ErrorKind::Other, ...)`

Modern Rust provides `io::Error::other()` as a cleaner API.

**`persistence/backend.rs:57`**
```rust
// Before
BackendError::Other(e) => io::Error::new(io::ErrorKind::Other, e),

// After
BackendError::Other(e) => io::Error::other(e),
```

**`persistence/native.rs` (5 locations: lines 36, 51, 66, 78, 92)**
```rust
// Before
Err(std::io::Error::new(std::io::ErrorKind::Other, "lock poisoned"))

// After
Err(std::io::Error::other("lock poisoned"))
```

### 2. Add `is_empty()` to `StorageFile` trait

Clippy warns when a type has `len()` but no `is_empty()`.

**`persistence/backend.rs:67-82`**
```rust
pub trait StorageFile: Send + Sync {
    // ... existing methods ...

    fn len(&self) -> BoxFuture<'_, Result<u64, BackendError>>;

    // Add default implementation
    fn is_empty(&self) -> BoxFuture<'_, Result<bool, BackendError>> {
        Box::pin(async { Ok(self.len().await? == 0) })
    }
}
```

### 3. Explicit `truncate(false)` for file creation

When using `OpenOptions` with `.create(true)`, explicitly specify truncate behavior.

**`persistence/native.rs:154-158`**
```rust
// Before
let file = fs::File::options()
    .read(true)
    .write(true)
    .create(true)
    .open(path)?;

// After
let file = fs::File::options()
    .read(true)
    .write(true)
    .create(true)
    .truncate(false)
    .open(path)?;
```

### 4. Simplify dereference in `debug_shim.rs`

**`debug_shim.rs:40`**
```rust
// Before
Some(res) => DebugGizmos(Some(&*res))

// After
Some(ref res) => DebugGizmos(Some(res))
```

### 5. Dead code: `gen_single_2d`

Check if `gen_single_2d()` is used anywhere:
- `seeding/noise/native.rs:36-38`
- `seeding/noise/wasm.rs:69-72`

If unused, either:
- Remove the method entirely, or
- Add `#[allow(dead_code)]` with a comment explaining why it's kept

## Verification

```bash
cargo clippy -p game -- -D warnings
cargo build -p game
cargo test -p game
```

## Estimated Impact

- **Risk:** None - purely mechanical changes
- **Lines changed:** ~15
- **CI benefit:** Clean Clippy output
