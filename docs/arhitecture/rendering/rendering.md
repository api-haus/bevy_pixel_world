# Rendering Pipeline

Chunk texture management and material identity textures.

## Overview

The rendering system handles uploading pixel data to GPU textures and managing visual assets for materials.

## Chunk Rendering

Each chunk is rendered as a quad entity in world space using Bevy.

| Component          | Description                                                          |
|--------------------|----------------------------------------------------------------------|
| **Quad entity**    | World-space positioned quad per chunk                                |
| **Texture format** | 32-bit uncompressed (compatible with pixel data layout)              |
| **Surface shader** | Custom shader handling palette lookups, heat glow effects, wet sheen |

The shader reads pixel data from the chunk texture and applies visual effects based on flags and heat layer values.

## Chunk Texture Upload

Chunks are uploaded to GPU textures when their content changes.

### Layer Upload

Each layer can be uploaded independently based on its schedule:

| Layer | Default Schedule | Behavior |
|-------|------------------|----------|
| Color | `OnChange` | Upload when color data changes |
| Heat | `Periodic(4)` | Upload every 4th tick for glow effects |
| Material | — | Not uploaded directly (used for color lookup) |
| BrickLayer.id | `OnChange` | Upload when brick assignments change |
| BrickLayer.damage | `Periodic(4)` | Upload for damage visualization |

**Note:** The Color layer is optional. Without it, rendering derives color directly from material definitions in the material registry. See [Pixel Layers](../modularity/pixel-layers.md) for upload schedules.

### Brick Layer Rendering

When `BrickLayer` is registered, the shader combines both sub-layers for block-based damage visualization:

1. **Brick identification**: Sample `brick_id` at pixel position → get brick index (0-255 for GRID=16)
2. **Damage lookup**: Sample `damage` texture at brick index → get damage value (0-255)
3. **Visual effect**: Apply damage overlay (cracks, glow, desaturation) to entire brick region

```
// Shader pseudocode
brick_id = brick_id_texture[pixel_uv];
damage = damage_texture[brick_id];
crack_intensity = damage / 255.0;
final_color = mix(base_color, crack_color, crack_intensity);
```

The damage texture is a 1D lookup (GRID² entries), not a 2D spatial texture. This ensures all pixels in a brick show identical damage effects regardless of hit location.

### Whole-Chunk Upload Strategy

The current approach uploads entire chunk textures when any pixel within the chunk changes:

| Aspect         | Description                             |
|----------------|-----------------------------------------|
| **Trigger**    | Any pixel modification within the chunk |
| **Scope**      | Entire layer buffer uploaded            |
| **Simplicity** | No partial update tracking needed       |

**Rationale:**

- Simple implementation with predictable performance
- Avoids complexity of tracking sub-chunk dirty regions for GPU upload
- Modern GPUs handle texture uploads efficiently
- Chunk sizes are designed to balance memory and upload cost

### Partial Uploads (Not Implemented)

Partial texture uploads are **not implemented** in the current design.

**Rationale:**

- Whole-chunk uploads are simpler and more predictable
- Modern GPU texture upload bandwidth is sufficient for chunk-sized textures
- CPU overhead of tracking sub-chunk dirty regions outweighs upload savings
- Chunk sizes are tuned to balance memory footprint with upload cost

This may be revisited if profiling reveals texture upload as a bottleneck.

## Material Identity Textures

Materials can optionally have associated identity textures for visual richness. This is an advanced feature for future
PCG integration.

### Concept

| Component            | Description                                                |
|----------------------|------------------------------------------------------------|
| **Identity texture** | Tileable small pixel-art PNG asset                         |
| **Location**         | Stored in assets directory                                 |
| **Optional**         | Materials may or may not have an identity texture assigned |
| **Purpose**          | Visual aesthetic guiding pixel coloration during seeding   |

### Seeding Integration

When the chunk seeder places a material with an identity texture:

1. Sample the identity texture at the pixel's local position (tiled/wrapped)
2. Use sampled color as the pixel's `color` field (palette index)
3. Result: natural variation within material regions

### Example Materials

| Material | Identity Texture        | Effect                   |
|----------|-------------------------|--------------------------|
| Stone    | Grainy noise pattern    | Natural rock variation   |
| Brick    | Repeating brick pattern | Structured appearance    |
| Wood     | Grain lines             | Directional wood texture |

**Note:** Identity textures are part of the advanced PCG pipeline. See [PCG World Ideas](../world-generation/pcg-ideas.md) for integration
with stamps and WFC-based generation.

## Related Documentation

- [Pixel Camera](pixel-camera.md) - Camera snapping and subpixel offset
- [Pixel Format](../foundational/pixel-format.md) - Color field storing palette index
- [Chunk Seeding](../chunk-management/chunk-seeding.md) - Where identity textures are applied
- [Materials](../simulation/materials.md) - Material definitions
- [Spatial Hierarchy](../foundational/spatial-hierarchy.md) - Chunk organization
- [Architecture Overview](../README.md)
