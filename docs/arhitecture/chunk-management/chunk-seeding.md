# Chunk Seeding

Trait abstraction for populating empty chunks with initial pixel data.

## Overview

When a chunk is assigned to a new world position, it needs initial pixel data. The **ChunkSeeder** trait provides a
pluggable interface for generating this data, supporting both procedural generation and disk persistence.

## ChunkSeeder Trait

The seeder trait defines how chunk buffers are populated:

```mermaid
flowchart TB
    subgraph Trait["ChunkSeeder Trait"]
        direction TB
        Input["Input: world position, buffer reference"]
        Output["Output: populated buffer, completion signal"]
        Input --> Process["seed(chunk_pos, buffer)"]
        Process --> Output
    end
```

| Method     | Purpose                                              |
|------------|------------------------------------------------------|
| `seed`     | Fill buffer with pixel data for given world position |
| `is_async` | Whether seeding may block (disk I/O)                 |

## Implementation: Noise Seeder

Procedural terrain generation using coherent noise.

```mermaid
flowchart LR
    subgraph NoiseSeeder["Noise Seeder"]
        direction TB
        Pos["World Position"] --> Hash["Deterministic Seed"]
        Hash --> Noise["fastnoise2-rs"]
        Noise --> Terrain["Terrain Features"]
    end

    Terrain --> Caves["Caves"]
    Terrain --> Layers["Sediment Layers"]
    Terrain --> Ores["Ore Veins"]
```

### Characteristics

| Property      | Value                                            |
|---------------|--------------------------------------------------|
| Deterministic | Same world position always produces same terrain |
| Infinite      | Any world coordinate can be generated            |
| Stateless     | No disk I/O required                             |
| Fast          | Suitable for real-time generation                |

### Terrain Generation Pipeline

A basic proof-of-concept pipeline for initial development:

1. **Height map** - 2D noise determines surface elevation
2. **Layer placement** - Depth-based material assignment (dirt, stone, bedrock)
3. **Cave carving** - 3D noise creates underground cavities (optional)

This serves as a minimal seeder example. Production-quality worlds may use advanced techniques like WFC, stamps, and hierarchical content (see docs/ideas).

### Noise Configuration

| Noise Type | Use Case                         |
|------------|----------------------------------|
| Perlin     | Smooth terrain elevation         |
| Simplex    | Cave systems, organic shapes     |
| Cellular   | Ore clusters, crystal formations |
| Value      | Background variation             |

## Implementation: Persistence Seeder

Disk-based storage for modified chunks. Wraps another seeder and checks disk before delegating.

```mermaid
flowchart TB
    subgraph PersistenceSeeder["Persistence Seeder"]
        direction TB
        Pos["World Position"] --> Check{Saved on disk?}
        Check -->|" yes "| Load["Load from disk"]
        Check -->|" no "| Fallback["Delegate to inner seeder"]
        Load --> Buffer["Populate buffer"]
        Fallback --> Buffer
    end
```

The persistence seeder is a decorator—it wraps a noise seeder (or any other seeder) and intercepts requests to check
for saved data first.

See [Chunk Persistence](../persistence/chunk-persistence.md) for:

- Save file binary format
- Page table structure for random access
- Compression strategy (LZ4, delta encoding)
- Dirty tracking and write paths
- Large file optimizations

## Seeder Composition

Multiple seeders can be chained with fallback behavior:

```mermaid
flowchart TB
    Request["Seed Request"] --> P["Persistence Seeder"]
    P -->|" found on disk "| Done["Buffer Populated"]
    P -->|" not found "| N["Noise Seeder"]
    N --> Done
```

This enables:

- Player modifications persist to disk
- Unvisited areas generate procedurally
- Seamless transition between saved and generated content

## Async Seeding

Seeders that perform I/O (disk, network) should be async to avoid blocking the main thread:

```mermaid
flowchart LR
    subgraph Main["Main Thread"]
        Request["Request seed"]
        Receive["Receive seeded chunk"]
    end

    subgraph Background["Background Thread"]
        Queue["Seed queue"]
        Generate["Generate/Load"]
    end

    Request -->|"enqueue"| Queue
    Queue --> Generate
    Generate -->|"complete"| Receive
```

The streaming window requests chunks ahead of the camera, hiding generation/load latency.

### Seeder Threading Model

| Seeder Type | Threading           | Reason                                 |
|-------------|---------------------|----------------------------------------|
| Noise       | Background pool     | CPU-bound, benefits from parallelism   |
| Persistence | Dedicated I/O       | Disk-bound, avoid head contention      |
| Hybrid      | I/O with CPU assist | Check disk, then parallel generate     |

## Pixel Body Preservation

When async seeding completes, existing pixel body data in the chunk must not be overwritten. The `merge_seeded_pixels()`
function handles this by preserving pixels flagged as belonging to a body.

### The Problem

Seeding is asynchronous—a chunk may be requested, and while seeding runs on a background thread:
1. A pixel body loads and blits into the chunk slot
2. Seeding completes with fresh terrain data
3. **Bug if naive merge**: terrain overwrites body pixels, body appears destroyed

### The Solution

Check the `PIXEL_BODY` flag before overwriting:

```mermaid
flowchart TD
    subgraph Merge["merge_seeded_pixels()"]
        direction TB
        ForEach["For each pixel position"]
        Check{Existing pixel has PIXEL_BODY flag?}
        Keep["Keep existing pixel"]
        Overwrite["Write seeded pixel"]

        ForEach --> Check
        Check -->|"yes"| Keep
        Check -->|"no"| Overwrite
        Keep --> ForEach
        Overwrite --> ForEach
    end
```

### Behavior

| Existing Pixel | Seeded Pixel | Result |
|----------------|--------------|--------|
| Has `PIXEL_BODY` flag | Any | Keep existing (body pixel preserved) |
| No `PIXEL_BODY` flag | Any | Write seeded pixel |
| Void | Any | Write seeded pixel |

This ensures pixel bodies remain intact even when their chunks are being seeded asynchronously. The body's pixels take
priority over procedural terrain.

## Surface Distance Coloring

A technique for assigning materials based on distance to air:

```mermaid
flowchart LR
    subgraph DistanceColoring["Surface Distance"]
        direction TB
        A["Generate solid/air mask"]
        B["Compute distance to nearest air"]
        C["Map distance to material"]
        A --> B --> C
    end

    C --> Soil["0-3 pixels: Soil"]
    C --> Stone["4+ pixels: Stone"]
```

This creates natural-looking terrain with surface soil transitioning to deeper stone.

### Algorithm

1. Generate noise, threshold to solid/air
2. For each solid pixel, calculate distance to nearest air (flood fill or jump flood)
3. Map distance ranges to materials

### Color Variation

Within each material, use the pixel's `ColorIndex` field for variation:

- Sample secondary noise at pixel position
- Map to palette index (0-255)
- Material's palette provides actual RGB values

This prevents flat, uniform terrain while keeping material identity consistent.

## Related Documentation

- [Chunk Persistence](../persistence/chunk-persistence.md) - Save/load system for modified chunks
- [Chunk Pooling](chunk-pooling.md) - Lifecycle that triggers seeding
- [Streaming Window](../streaming/streaming-window.md) - Determines which chunks need seeding
- [Materials](../simulation/materials.md) - Material definitions for seeded pixels
- [Configuration Reference](../foundational/configuration.md) - Seeder parameters
- [Architecture Overview](../README.md)
