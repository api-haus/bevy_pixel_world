# Coordinate System Convention

The canonical coordinate system convention used throughout the project.

## Overview

All layers of the system use a unified **Y+ up** coordinate convention:

| Axis   | Direction          |
|--------|--------------------|
| **X+** | Right (east)       |
| **Y+** | Up (toward sky)    |
| Origin | Bottom-left corner |

This convention applies consistently to:

- World coordinates
- Chunk addressing
- Pixel/local coordinates within chunks
- Surface storage (row 0 = bottom row)
- API parameters and return values

## Why Y+ Up

The Y+ up convention aligns with:

1. **Mathematical convention** - Standard Cartesian coordinates in mathematics and physics
2. **Bevy world coordinates** - Bevy's 2D world space uses Y+ up
3. **Intuitive reasoning** - "Up" meaning increasing Y matches natural language
4. **Physics simulation** - Gravity pointing toward -Y is natural

This eliminates cognitive overhead when working across system layers.

## Layer-by-Layer Implementation

### World Coordinates

Global pixel addressing uses signed integers with Y+ up.

```
         +Y (up)
           ^
           |
           |
  -X <-----+-----> +X (right)
           |
           |
         -Y (down)
```

### Chunk Coordinates

Chunks are identified by their position in chunk-space (world position divided by `CHUNK_SIZE`).

- Chunk (0, 0) covers world pixels (0, 0) to (`CHUNK_SIZE`-1, `CHUNK_SIZE`-1)
- Chunk (0, 1) is directly above chunk (0, 0)
- Chunk (1, 0) is directly to the right of chunk (0, 0)

### Local Coordinates (Pixel within Chunk)

Local coordinates range from (0, 0) to (`CHUNK_SIZE`-1, `CHUNK_SIZE`-1).

- (0, 0) is the **bottom-left** pixel of the chunk
- (`CHUNK_SIZE`-1, `CHUNK_SIZE`-1) is the **top-right** pixel

### Surface Storage

The `Surface` struct stores pixels in row-major order with **row 0 as the bottom row**:

```
Memory layout:        Visual representation:

data[3*w .. 4*w-1]    Row 3 (top)
data[2*w .. 3*w-1]    Row 2
data[1*w .. 2*w-1]    Row 1
data[0   .. w-1]      Row 0 (bottom)
```

Buffer index calculation:

```
index = y * width + x
```

Where `y=0` is the bottom row.

### GPU Texture Upload

GPU textures typically expect row 0 at the **top** (image convention), but our surface stores row 0 at the **bottom** (
world convention).

The correction is handled at **mesh initialization** via `create_chunk_quad()`, which constructs UV coordinates with
(0, 0) at the bottom-left vertex instead of the top-left. This allows the shader to sample directly without any
transformation:

```wgsl
// chunk.wgsl
return textureSample(chunk_texture, chunk_sampler, mesh.uv);
```

### UV Coordinates

Bevy's default `Rectangle` mesh has UV (0, 0) at top-left. We use `create_chunk_quad()` which assigns UVs directly
matching world coordinates:

| Vertex       | UV     | World Position |
|--------------|--------|----------------|
| Bottom-left  | (0, 0) | Bottom-left    |
| Bottom-right | (1, 0) | Bottom-right   |
| Top-right    | (1, 1) | Top-right      |
| Top-left     | (0, 1) | Top-left       |

## Diagram

```
World Space (Y+ up)

      Chunk (0, 1)           Chunk (1, 1)
    +-------------+        +-------------+
    |             |        |             |
    |   (0,h-1)---|--------|---(w-1,h-1) |
    |     ^       |        |       ^     |
    |     |       |        |       |     |
    |   Y+|       |        |       |     |
    |     |       |        |       |     |
    |   (0,0)-----|-----X+-|-->(w-1,0)   |
    +-------------+        +-------------+
      Chunk (0, 0)           Chunk (1, 0)
           |
           +-- Origin (world 0, 0)
```

## Implementation Notes

### Where the UV Correction Happens

The coordinate convention flows cleanly through the entire system without runtime transformation:

| Layer             | Convention | Transformation  |
|-------------------|------------|-----------------|
| World coordinates | Y+ up      | None            |
| Chunk coordinates | Y+ up      | None            |
| Local coordinates | Y+ up      | None            |
| Surface buffer    | Y+ up      | None            |
| Texture upload    | Y+ up      | None            |
| **Mesh UVs**      | Y+ up      | **Set at init** |
| Shader sampling   | Y+ up      | None            |
| Screen output     | Y+ up      | None            |

The UV correction is done once at mesh creation time via `create_chunk_quad()` in `render.rs`. The shader samples
directly without any coordinate manipulation.

### Benefits

- Zero per-frame transformation cost
- No Y-flipping during simulation or blitting
- API surface is consistent and predictable
- Shader code is simple and straightforward
- Debug visualization matches world coordinates

## Related Documentation

- [Spatial Hierarchy](spatial-hierarchy.md) - World, chunk, tile, pixel organization
- [Rendering](../rendering/rendering.md) - Chunk texture upload
- [Architecture Overview](../README.md)
