# Chunk Pooling

Object pool pattern for zero-allocation chunk management.

## Design Goal

Eliminate runtime memory allocations during gameplay. All chunk buffers are allocated once at startup and reused
throughout the session. This prevents allocation spikes, fragmentation, and garbage collection pauses.

## Pool Structure

The pool consists of:

- **Fixed chunk count** - `POOL_SIZE` (= `WINDOW_WIDTH * WINDOW_HEIGHT`) chunks allocated at startup
- **Uniform chunk buffers** - Each chunk is `CHUNK_SIZE` × `CHUNK_SIZE` pixels
- **Pre-allocated memory** - `POOL_SIZE * CHUNK_SIZE * CHUNK_SIZE * 4` bytes total

See [Configuration Reference](configuration.md) for compile-time constants.

## Chunk Memory Layout

Each chunk stores its data as separate linear arrays, one per simulation layer:

| Layer  | Element Type      | Size                                   |
|--------|-------------------|----------------------------------------|
| Pixels | 4 bytes per pixel | `CHUNK_SIZE × CHUNK_SIZE × 4`          |
| Heat   | u8 per cell       | `(CHUNK_SIZE / 4) × (CHUNK_SIZE / 4)`  |

Future layers (moisture, pressure, etc.) follow the same pattern. Additional layers planned for Phase 7: moisture (full resolution), pressure (4x downsampled like heat). See plan.md Phase 7. Each layer is a contiguous array stored alongside the chunk metadata.

## Chunk Lifecycle

```mermaid
stateDiagram-v2
    [*] --> InPool: startup allocation
    InPool --> Seeding: assigned to world position
    Seeding --> Active: data ready
    Active --> Active: simulation tick
    Active --> Recycling: camera moves away
    Recycling --> InPool: buffer cleared
```

## State Descriptions

### In Pool

- Buffer memory is allocated but unassigned
- No world position associated
- Ready for immediate assignment without allocation

### Seeding

- Assigned to a specific world coordinate (chunk position)
- Chunk seeder is filling the buffer with initial data
- May be async if loading from disk

### Active

- Fully initialized and part of the active region
- Participates in simulation each tick
- Rendered to the screen
- May be modified by player interaction

### Recycling

- Camera has moved, chunk is no longer in active region
- If modified, optionally persisted to disk
- Buffer contents cleared (zeroed or marked invalid)
- Returns to pool for reuse

## Buffer Reuse Pattern

When a chunk is recycled:

```mermaid
flowchart LR
    A[Active Chunk] -->|" camera moves "| B{Modified?}
    B -->|" yes "| C[Persist to Disk]
    B -->|" no "| D[Clear Buffer]
    C --> D
    D --> E[Return to Pool]
    E -->|" new assignment "| F[Seeding]
```

The buffer is never deallocated. Instead:

1. **Clear** - Zero the pixel buffer or mark as uninitialized
2. **Reassign** - Update the chunk's world position
3. **Refill** - Chunk seeder writes new data into the same buffer

## Benefits

- **Predictable memory usage** - No allocation spikes during exploration
- **No fragmentation** - Fixed-size buffers prevent heap fragmentation
- **Deterministic performance** - No garbage collection or allocator latency

## Related Documentation

- [Streaming Window](streaming-window.md) - Decides when chunks enter/leave the pool
- [Chunk Seeding](chunk-seeding.md) - Fills pooled chunks with data
- [Chunk Persistence](chunk-persistence.md) - Saves modified chunks during recycling
- [Configuration Reference](configuration.md) - Pool size and chunk parameters
- [Architecture Overview](README.md)
