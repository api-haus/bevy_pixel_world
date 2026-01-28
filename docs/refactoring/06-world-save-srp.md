# WorldSave Single Responsibility (Deferred)

Large refactor to separate concerns in `WorldSave`. Deferred for a dedicated work session.

## Current State

`WorldSave` handles multiple responsibilities:
1. Chunk persistence (save/load chunks)
2. Body persistence (save/load pixel bodies)
3. Time tracking (save timestamps)
4. File format management
5. Compression

## Why Defer

- Significant scope (~200+ lines affected)
- Requires careful migration of existing save files
- Benefits are architectural, not immediate
- Current implementation works correctly

## Proposed Extraction

### 1. `ChunkPersistence`

Handles saving/loading world chunks:
```rust
pub struct ChunkPersistence {
    backend: Box<dyn StorageBackend>,
    compression: CompressionLevel,
}

impl ChunkPersistence {
    pub async fn save_chunk(&self, pos: ChunkPos, data: &ChunkData) -> Result<()>;
    pub async fn load_chunk(&self, pos: ChunkPos) -> Result<Option<ChunkData>>;
    pub async fn delete_chunk(&self, pos: ChunkPos) -> Result<()>;
}
```

### 2. `BodyPersistence`

Handles saving/loading pixel bodies:
```rust
pub struct BodyPersistence {
    backend: Box<dyn StorageBackend>,
}

impl BodyPersistence {
    pub async fn save_body(&self, id: PixelBodyId, record: &PixelBodyRecord) -> Result<()>;
    pub async fn load_body(&self, id: PixelBodyId) -> Result<Option<PixelBodyRecord>>;
    pub async fn list_bodies(&self) -> Result<Vec<PixelBodyId>>;
}
```

### 3. Time Provider Abstraction

For testability:
```rust
pub trait TimeProvider: Send + Sync {
    fn now(&self) -> SystemTime;
}

pub struct SystemTimeProvider;
impl TimeProvider for SystemTimeProvider {
    fn now(&self) -> SystemTime { SystemTime::now() }
}

#[cfg(test)]
pub struct MockTimeProvider { pub fixed_time: SystemTime }
```

### 4. Refactored WorldSave

```rust
pub struct WorldSave {
    chunks: ChunkPersistence,
    bodies: BodyPersistence,
    time: Box<dyn TimeProvider>,
    metadata: WorldMetadata,
}
```

## Migration Considerations

- Existing save files must continue to work
- Version field in metadata for format evolution
- Gradual migration on load (upgrade format silently)

## Prerequisites

Before starting:
1. Complete 01-05 refactorings (clean foundation)
2. Add comprehensive persistence tests
3. Document current file format

## Verification (When Implemented)

```bash
cargo clippy -p bevy_pixel_world -- -D warnings
cargo build -p bevy_pixel_world
cargo test -p bevy_pixel_world

# Critical: Verify save/load roundtrip
cargo run --example persistence_test
```

## Estimated Impact

- **Risk:** High - touches core persistence
- **Lines changed:** ~300
- **Time:** Dedicated work session
- **Benefit:** Better testability, clearer architecture
