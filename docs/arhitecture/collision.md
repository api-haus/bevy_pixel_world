# Collision System

Mesh generation from solid pixels for physics interactions.

## Overview

The collision system generates collision meshes from pixel data, enabling physics entities to interact with the
simulated terrain.

## Collision Pixel Selection

For collision mesh generation, pixels must meet **both** criteria:

| Flag      | Required Value | Meaning                                                     |
|-----------|----------------|-------------------------------------------------------------|
| `solid`   | 1              | Material is not liquid or gas (includes solids and powders) |
| `falling` | 0              | Pixel is stable (at rest, not currently moving)             |

**Selection logic:** `solid=1 AND falling=0`

| Pixel State    | `solid` | `falling` | In Collision Mesh? |
|----------------|---------|-----------|--------------------|
| Settled stone  | 1       | 0         | Yes                |
| Falling debris | 1       | 1         | No (still moving)  |
| Settled sand   | 1       | 0         | Yes                |
| Falling sand   | 1       | 1         | No (still moving)  |
| Water          | 0       | 0         | No (liquid)        |
| Steam          | 0       | 0         | No (gas)           |

**Note:** The `solid` flag caches whether the material's `state` is `solid` or `powder` (i.e., not `liquid` or `gas`).
This avoids a material registry lookup during collision mesh generation - a cache locality optimization since we're
iterating over many pixels.

## Mesh Generation Pipeline

The collision mesh is generated through a multi-stage pipeline:

```mermaid
flowchart LR
    Pixels["Solid Pixels"] --> MS["Marching Squares"]
    MS --> Outline["Outline Polygon"]
    Outline --> Simplify["Line Simplification"]
    Simplify --> Triangulate["Delaunay Triangulation"]
    Triangulate --> Mesh["Collision Mesh"]
```

### Stage 1: Marching Squares

Build outline polygons from the solid pixel grid.

| Aspect         | Description                                       |
|----------------|---------------------------------------------------|
| **Input**      | Binary grid (solid vs non-solid pixels)           |
| **Output**     | Contour polylines tracing solid boundaries        |
| **Algorithm**  | Standard marching squares with edge interpolation |
| **Resolution** | Per-pixel (no downsampling)                       |

**Note:** Per-pixel resolution is required for accurate collision meshes. Downsampling would compromise collision
fidelity.

### Stage 2: Line Simplification

Reduce vertex count while preserving shape.

| Aspect        | Description                              |
|---------------|------------------------------------------|
| **Input**     | Raw contour polylines                    |
| **Output**    | Simplified polylines with fewer vertices |
| **Algorithm** | Douglas-Peucker                          |

Tolerance value to be determined during Phase 5 implementation. Start with 1.0 pixel tolerance and tune based on visual/performance testing.

### Stage 3: Delaunay Triangulation

Produce optimized triangle mesh for physics engine.

| Aspect          | Description                                    |
|-----------------|------------------------------------------------|
| **Input**       | Simplified polygon outlines                    |
| **Output**      | Triangle mesh suitable for collision detection |
| **Constraints** | Respects polygon boundaries, no slivers        |

## Generation Scope and Locality

Collision meshes are **not** generated for the entire world. Instead:

### Dynamic Object Proximity

Meshes are generated only around **dynamic physics objects**:

| Object Type   | Example                          |
|---------------|----------------------------------|
| Characters    | Player, NPCs                     |
| Dynamic props | Falling crates, rolling boulders |

Areas without nearby dynamic objects have no collision mesh - there's nothing to collide with.

### Tile-Based Generation

Collision meshes are generated at **tile granularity**, not chunk granularity:

| Aspect          | Tile-Based Approach                                               |
|-----------------|-------------------------------------------------------------------|
| **Scope**       | Individual tiles around dynamic objects                           |
| **Consistency** | Tile boundaries provide natural mesh seams                        |
| **Efficiency**  | Avoids regenerating entire chunk when only a small region changes |

```
+-------+-------+-------+-------+
|       |       |       |       |
|       | [obj] |       |       |   [obj] = dynamic object
|       |       |       |       |
+-------+-------+-------+-------+
|       |#######|#######|       |   ####### = tiles with collision
|       |#######|#######|       |            mesh generated
|       |#######|#######|       |
+-------+-------+-------+-------+
|       |#######|#######|       |
|       |#######|#######|       |
|       |#######|#######|       |
+-------+-------+-------+-------+
```

### Update Triggers

Tile collision meshes are regenerated when:

1. A dynamic object enters a tile's proximity
2. Pixels within an active tile change (`solid` or `falling` flags modified)

Tiles outside any dynamic object's proximity have their meshes discarded.

### Update Frequency

Collision mesh updates run at **60 TPS** (same rate as CA and physics simulation), triggered by:

- Physics object proximity changes (object enters/leaves tile)
- Cellular automata pixel changes (solid/falling flag modifications)

See [Configuration](configuration.md) for tick rate definitions.

## Related Documentation

- [Pixel Format](pixel-format.md) - Solid flag definition
- [Simulation](simulation.md) - When solid flag changes
- [Spatial Hierarchy](spatial-hierarchy.md) - Chunk-based organization
- [Architecture Overview](README.md)
