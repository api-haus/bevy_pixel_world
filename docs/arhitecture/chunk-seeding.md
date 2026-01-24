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

This serves as a minimal seeder example. For production-quality worlds, see [PCG World Ideas](pcg-ideas.md) for advanced
generation with WFC, stamps, and hierarchical content.

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

See [Chunk Persistence](chunk-persistence.md) for:

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

- [Chunk Persistence](chunk-persistence.md) - Save/load system for modified chunks
- [Chunk Pooling](chunk-pooling.md) - Lifecycle that triggers seeding
- [Streaming Window](streaming-window.md) - Determines which chunks need seeding
- [PCG World Ideas](pcg-ideas.md) - Advanced generation with stamps and WFC
- [Materials](materials.md) - Material definitions for seeded pixels
- [Configuration Reference](configuration.md) - Seeder parameters
- [Architecture Overview](README.md)
