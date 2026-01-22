# Streaming Window

Camera-tracking region for infinite world exploration.

## Overview

The **streaming window** defines which chunks are currently loaded and active. It follows the camera as the player
explores, recycling chunks behind the camera and assigning them to positions ahead.

The terms "streaming window" and "active region" are used interchangeably throughout the documentation:

- **Streaming window** emphasizes the loading/unloading mechanism
- **Active region** emphasizes the set of currently loaded chunks

## Active Region Structure

The active region (streaming window) is defined by:

- **Window dimensions** - Width and height in chunks
- **Total chunk count** - Must match the chunk pool size
- **World coverage** - Window dimensions × chunk dimensions in pixels

See [Configuration Reference](configuration.md) for tunable parameters.

## Camera Tracking

```mermaid
flowchart TB
    Start[Camera Position Update] --> Check{Crossed chunk boundary?}
    Check -->|" no "| End[No action needed]
    Check -->|" yes "| Calc[Calculate new window center]
    Calc --> Diff[Determine chunks to recycle]
    Diff --> Recycle[Recycle identified chunks]
    Recycle --> Assign[Assign to new world positions]
    Assign --> Seed[Queue for seeding]
    Seed --> End
```

### Boundary Detection

The window center is quantized to chunk coordinates. When the camera moves far enough that the center should shift:

1. **Calculate delta** - How many chunks the window needs to shift (usually 1)
2. **Identify obsolete chunks** - Chunks that will fall outside the new window bounds
3. **Determine new positions** - World coordinates for recycled chunks
4. **Prioritize by direction** - Chunks ahead of camera movement load first

## Hysteresis Buffer

Prevents rapid recycling when the camera oscillates near a chunk boundary:

```mermaid
flowchart LR
    subgraph Hysteresis["Boundary Hysteresis"]
        direction TB
        A["Camera at boundary"] --> B{Movement direction stable?}
        B -->|" no (oscillating) "| C[Suppress recycle]
        B -->|" yes (committed) "| D[Proceed with recycle]
        C --> E[Wait for stable movement]
        E --> B
    end
```

**Why hysteresis matters:**

- Camera jitter at boundaries would cause constant chunk recycling
- Recycling is expensive (disk I/O, seeding computation)
- Small buffer zone absorbs oscillation without triggering recycling

### Implementation

- Track camera velocity direction over several frames
- Only trigger recycling when direction is consistent
- Configurable threshold for direction stability [needs clarification: threshold value]

## Loading Priority

When multiple chunks need loading, prioritize by:

```mermaid
flowchart TB
    subgraph Priority["Loading Priority"]
        direction TB
        P1["1. Directly ahead of camera"]
        P2["2. Adjacent to camera direction"]
        P3["3. Perpendicular to camera direction"]
        P4["4. Behind camera (low priority)"]
        P1 --> P2 --> P3 --> P4
    end
```

| Priority | Chunks           | Rationale                   |
|----------|------------------|-----------------------------|
| Highest  | Directly ahead   | Player will see these first |
| High     | Diagonally ahead | Likely visible soon         |
| Medium   | Perpendicular    | May become visible          |
| Low      | Behind           | Player moving away          |

## Window Visualization

```
        Camera movement →

    +---+---+---+---+---+---+
    | L | L | . | . | . | N |  L = Low priority (recycle candidates)
    +---+---+---+---+---+---+  N = New (high priority load)
    | L | L | . | . | . | N |  . = Active chunks
    +---+---+---+---+---+---+
    | L | L | . | [C] | . | N |  [C] = Camera position
    +---+---+---+---+---+---+
    | L | L | . | . | . | N |
    +---+---+---+---+---+---+

```

As the camera moves right:

- Left column (L) becomes candidates for recycling
- Right column (N) represents new world positions to load
- Center chunks remain active

## Related Documentation

- [Chunk Pooling](chunk-pooling.md) - Provides chunks for the streaming window
- [Chunk Seeding](chunk-seeding.md) - Initializes newly assigned chunks
- [Configuration Reference](configuration.md) - Window and hysteresis parameters
- [Architecture Overview](README.md)
