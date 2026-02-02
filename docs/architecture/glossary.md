# Glossary

Canonical definitions of technical terms used throughout the architecture documentation.

## Abbreviations

| Abbreviation | Expansion                     | Context                                |
|--------------|-------------------------------|----------------------------------------|
| **CA**       | Cellular Automata             | Grid-based simulation model            |
| **TPS**      | Ticks Per Second              | Fixed update rate for simulation loops |
| **WFC**      | Wave Function Collapse        | Constraint-based procedural generation |
| **PCG**      | Procedural Content Generation | Algorithmic world/content creation     |

---

## Spatial Hierarchy

Terms describing the four-level spatial organization.

| Term      | Definition                                                                                     | Documentation                                |
|-----------|------------------------------------------------------------------------------------------------|----------------------------------------------|
| **World** | Infinite 2D coordinate space providing global addressing. Has no direct memory representation. | [spatial-hierarchy.md](foundational/spatial-hierarchy.md) |
| **Chunk** | Fixed-size rectangular pixel buffer. Unit of pooling, streaming, persistence, and rendering.   | [spatial-hierarchy.md](foundational/spatial-hierarchy.md) |
| **Tile**  | Subdivision of a chunk used for checkerboard scheduling and dirty rect tracking.               | [spatial-hierarchy.md](foundational/spatial-hierarchy.md) |
| **Pixel** | Fundamental simulation unit. 4-byte struct (material, color, damage, flags) stored in chunks. | [pixel-layers.md](modularity/pixel-layers.md) |

### Coordinate Systems

| Term                  | Range                  | Usage                                      |
|-----------------------|------------------------|--------------------------------------------|
| **World coordinates** | Infinite (signed)                 | Global pixel addressing                    |
| **Chunk coordinates** | Infinite (signed)                 | Identifies which chunk contains a position |
| **Tile coordinates**  | 0 to `CHUNK_SIZE/TILE_SIZE - 1`   | Position within chunk's tile grid          |
| **Local coordinates** | 0 to `CHUNK_SIZE - 1`             | Pixel position within a chunk              |

---

## Material System

Terms related to material definitions and behavior.

| Term                  | Definition                                                                                                          | Documentation                |
|-----------------------|---------------------------------------------------------------------------------------------------------------------|------------------------------|
| **Material**          | Type definition controlling pixel behavior. Indexed by u8 ID into a registry of up to 256 materials.                | [materials.md](simulation/materials.md) |
| **Material Registry** | Array storing all material definitions, indexed by Material ID.                                                     | [materials.md](simulation/materials.md) |
| **void**              | Reserved Material ID 0 representing empty space.                                                                    | [materials.md](simulation/materials.md) |
| **Tag**               | Category label for interaction targeting (e.g., `stone`, `organic`, `flammable`). Materials can have multiple tags. | [materials.md](simulation/materials.md) |
| **Interaction**       | Definition of what happens when materials contact: corrode, ignite, diffuse, transform, displace, or none.          | [materials.md](simulation/materials.md) |

### Behavior Type (Aggregate State)

The four material behavior types have special status: they must be specified both as the `state` property AND as the
first tag on every material. This dual specification enables efficient simulation dispatch and flexible tag-based
interaction targeting.

| Behavior Type | Movement Rules                                   | Examples                  |
|---------------|--------------------------------------------------|---------------------------|
| **solid**     | Static; does not move; supports neighbors        | Stone, metal, wood, brick |
| **powder**    | Falls down; piles up; slides off slopes          | Sand, gravel, ash, soil   |
| **liquid**    | Falls down; flows horizontally; fills containers | Water, oil, lava, acid    |
| **gas**       | Rises; disperses in all directions               | Steam, smoke, fog         |

**Note:** Specific materials like "dust" or "sand" are materials with behavior type `powder`, not behavior types
themselves. See [materials.md](simulation/materials.md) for the full convention.

---

## Layer System

Two storage patterns for per-pixel data:

1. **Pixel struct (AoS):** Main struct, swaps atomically
2. **Separate layers (SoA):** Additional arrays for spatial/downsampled data

### Core Concepts

| Term           | Definition                                                                                                      | Documentation                          |
|----------------|-----------------------------------------------------------------------------------------------------------------|----------------------------------------|
| **Pixel struct** | 4-byte struct containing all per-pixel fields. Stored in AoS layout.                                          | [pixel-layers.md](modularity/pixel-layers.md) |
| **Separate layer** | Optional SoA array for data with different lifetime/resolution than pixel struct.                            | [pixel-layers.md](modularity/pixel-layers.md) |
| **swap_follow** | Layer configuration: whether data moves with pixel swaps (true) or stays at location (false).                  | [pixel-layers.md](modularity/pixel-layers.md) |
| **sample_rate** | Layer resolution: 1 = per-pixel, 4 = 4×4 regions, etc. Only applies to separate layers.                        | [pixel-layers.md](modularity/pixel-layers.md) |

### Pixel Structure

| Field      | Type | Definition                                                                                           |
|------------|------|------------------------------------------------------------------------------------------------------|
| material   | u8   | Type identifier indexing into material registry.                                                     |
| color      | u8   | Palette index for rendering.                                                                         |
| damage     | u4   | Accumulated damage (0-15).                                                                           |
| variant    | u4   | Visual variant (0-15).                                                                               |
| flags      | u8   | Boolean flags (dirty, solid, falling, burning, etc.).                                                |

---

## Simulation Types

| Term              | Definition                                                                                                 | Documentation                          |
|-------------------|------------------------------------------------------------------------------------------------------------|----------------------------------------|
| **WorldPos**      | Global pixel coordinate in world space. Used by simulations for pixel addressing across chunks.            | [simulation-extensibility.md](modularity/simulation-extensibility.md) |
| **WorldFragment** | Context passed to simulation closures. Contains position and normalized coordinates.                       | [simulation-extensibility.md](modularity/simulation-extensibility.md) |
| **SimContext**    | Resource containing simulation tick, seed, and jitter for deterministic randomness.                        | [simulation-extensibility.md](modularity/simulation-extensibility.md) |

---

## Data Structures

Reusable primitives and abstractions used across subsystems.

| Term        | Definition                                                                                                                                 | Documentation                                |
|-------------|--------------------------------------------------------------------------------------------------------------------------------------------|----------------------------------------------|
| **Surface** | Generic 2D pixel buffer (`Surface<T>`) with width, height, and contiguous data. Used by both chunks and pixel bodies for pixel storage.   | [pixel-bodies.md](physics/pixel-bodies.md)           |
| **Canvas**  | Unified read/write interface spanning multiple chunks. Abstracts cross-chunk pixel access during CA simulation and pixel body operations. | [spatial-hierarchy.md](foundational/spatial-hierarchy.md) |

---

## Simulation

Terms related to the multi-pass simulation system.

| Term                        | Definition                                                                                          | Documentation                                |
|-----------------------------|-----------------------------------------------------------------------------------------------------|----------------------------------------------|
| **Cellular Automata (CA)**  | Grid-based simulation where pixels update based on neighbor rules and material state.               | [simulation.md](simulation/simulation.md)               |
| **Falling sand**            | Genre/technique for powder and liquid physics via cellular automata.                                | [simulation.md](simulation/simulation.md)               |
| **Tick**                    | One complete simulation cycle comprising all passes.                                                | [simulation.md](simulation/simulation.md)               |
| **TPS**                     | Ticks per second. Fixed update rates: CA/Particles/Collision=60, Decay=20, Heat=10.                 | [configuration.md](foundational/configuration.md)         |
| **Pass**                    | Single processing sweep: CA pass, particle pass, material interactions pass, decay pass, heat pass. | [simulation.md](simulation/simulation.md)               |
| **Checkerboard scheduling** | 2x2 phase pattern (A, B, C, D) enabling parallel tile processing without race conditions.           | [simulation.md](simulation/simulation.md)               |
| **Phase**                   | One of four checkerboard groups. Tiles of the same phase process in parallel.                       | [simulation.md](simulation/simulation.md)               |
| **Dirty rect**              | Per-tile bounding box for simulation scheduling. Distinct from per-pixel dirty flag.                | [spatial-hierarchy.md](foundational/spatial-hierarchy.md) |

### Simulation Layers

| Term           | Definition                                                                                         | Documentation                  |
|----------------|----------------------------------------------------------------------------------------------------|--------------------------------|
| **Heat layer** | Downsampled thermal map (`CHUNK_SIZE`/4 resolution). Stores u8 temperature values (0=cold, 255=hot). | [simulation.md](simulation/simulation.md) |
| **Decay**      | Probabilistic time-based material transformation independent of pixel activity.                    | [simulation.md](simulation/simulation.md) |

---

## Particle System

Terms for the free-form particle effects system.

| Term                 | Definition                                                                                          | Documentation                |
|----------------------|-----------------------------------------------------------------------------------------------------|------------------------------|
| **Particle**         | Free-form entity with sub-pixel position and velocity for dynamic effects (debris, pouring, gases). | [particles.md](simulation/particles.md) |
| **Emission**         | Pixel to particle transition triggered by explosion, pouring, or gas release.                       | [particles.md](simulation/particles.md) |
| **Deposition**       | Particle to pixel transition when settling or colliding with the grid.                              | [particles.md](simulation/particles.md) |
| **particle_gravity** | Material property controlling particle fall rate. Negative values cause rising (steam, smoke).      | [materials.md](simulation/materials.md) |

---

## Streaming & Memory

Terms for chunk lifecycle and memory management.

| Term                  | Definition                                                                                       | Documentation                              |
|-----------------------|--------------------------------------------------------------------------------------------------|--------------------------------------------|
| **Chunk Pool**        | Object pool of pre-allocated chunk buffers enabling zero runtime allocation.                     | [chunk-pooling.md](chunk-management/chunk-pooling.md)       |
| **Streaming window**  | Camera-tracking region that determines which chunks are loaded. Synonymous with "active region". | [streaming-window.md](streaming/streaming-window.md) |
| **Active region**     | Set of currently loaded chunks around the camera. Synonymous with "streaming window".            | [streaming-window.md](streaming/streaming-window.md) |
| **Seeding**           | Filling a chunk buffer with initial pixel data from noise or disk.                               | [chunk-seeding.md](chunk-management/chunk-seeding.md)       |
| **ChunkSeeder**       | Trait abstraction for populating chunks (noise seeder, persistence seeder).                      | [chunk-seeding.md](chunk-management/chunk-seeding.md)       |
| **Recycling**         | Returning a chunk to the pool when the camera moves away; optionally persists to disk.           | [chunk-pooling.md](chunk-management/chunk-pooling.md)       |
| **Hysteresis**        | Buffer preventing rapid chunk recycling when camera oscillates near boundaries.                  | [streaming-window.md](streaming/streaming-window.md) |
| **Delta persistence** | Optimization storing only differences from procedural generation instead of full buffers.        | [chunk-seeding.md](chunk-management/chunk-seeding.md)       |

### Persistence Control

On-demand save API and dynamic object persistence.

| Term                     | Definition                                                                                                                        | Documentation                              |
|--------------------------|-----------------------------------------------------------------------------------------------------------------------------------|--------------------------------------------|
| **PersistenceControl**   | Resource providing on-demand save requests and auto-save configuration. Entry point for triggering saves from game code.          | [chunk-persistence.md](persistence/chunk-persistence.md) |
| **PersistenceHandle**    | Handle returned by `save()`. Tracks completion via `is_complete()` polling or async `into_future()`.                              | [chunk-persistence.md](persistence/chunk-persistence.md) |
| **AutoSaveConfig**       | Configuration for periodic auto-saves: enabled flag and interval duration. Default: 60 seconds.                                   | [chunk-persistence.md](persistence/chunk-persistence.md) |
| **SimulationState**      | Resource controlling pause/resume. When paused: CA and physics stop, rendering continues, persistence can still run.              | [chunk-persistence.md](persistence/chunk-persistence.md) |
| **Blitted position save**| Pixel bodies save using `BlittedTransform` position, not current physics position. Prevents ghost pixels on restore.              | [pixel-bodies.md](physics/pixel-bodies.md)         |

---

## Procedural Generation

Terms for world generation systems.

| Term        | Definition                                                                           | Documentation                        |
|-------------|--------------------------------------------------------------------------------------|--------------------------------------|
| **PCG**     | Procedural Content Generation - algorithmic creation of world content.               | [chunk-seeding.md](chunk-management/chunk-seeding.md) |
| **Noise**   | Coherent random functions (Perlin, Simplex, Cellular, Value) for terrain generation. | [chunk-seeding.md](chunk-management/chunk-seeding.md) |
| **WFC**     | Wave Function Collapse - constraint-based generation for macro-level structure.      | (ideas)         |
| **Stamp**   | Preset formation (cave, tree, building) placed during generation via stencil masks.  | (ideas)         |
| **Stencil** | Shape mask defining where a stamp applies to the world.                              | (ideas)         |

---

## Rendering

Terms for the rendering pipeline.

| Term                 | Definition                                                                     | Documentation                      |
|----------------------|--------------------------------------------------------------------------------|------------------------------------|
| **Chunk texture**    | GPU texture uploaded from chunk pixel buffer for rendering.                    | [rendering.md](rendering/rendering.md)       |
| **Palette**          | Color lookup table (up to 256 colors) indexed by pixel Color field.            | [pixel-format.md](foundational/pixel-format.md) |
| **Identity texture** | PNG asset defining repeating visual pattern applied during chunk seeding.      | [rendering.md](rendering/rendering.md)       |
| **Heat glow**        | Visual tinting of hot non-flammable materials based on heat layer temperature. | [simulation.md](simulation/simulation.md)     |

---

## Pixel Body

Terms for dynamic physics objects with pixel content. See [pixel-bodies.md](physics/pixel-bodies.md) for full architecture.

| Term                   | Definition                                                                                                                                              | Documentation                        |
|------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------|--------------------------------------|
| **Pixel Body**         | Dynamic physics object whose visual representation consists of individual pixels that participate in CA simulation. Can burn, melt, split into fragments. | [pixel-bodies.md](physics/pixel-bodies.md)   |
| **Shape Mask**         | Bitmask tracking which local pixels belong to the object (1) versus void/world (0). Updated when pixels are destroyed or move away.                      | [pixel-bodies.md](physics/pixel-bodies.md)   |
| **Blit**               | Writing pixel body content to the Canvas at world-transformed positions before CA simulation.                                                            | [pixel-bodies.md](physics/pixel-bodies.md)   |
| **Clear**              | Removing pixel body pixels from the Canvas after CA simulation, using the blitted transform rather than current physics position.                        | [pixel-bodies.md](physics/pixel-bodies.md)   |
| **BlittedTransform**   | Stored transform from the last blit operation. Ensures clear removes pixels from correct positions even after physics has moved the body.                | [pixel-bodies.md](physics/pixel-bodies.md)   |
| **Inverse Transform**  | Blit technique that iterates world pixels in the AABB and maps each back to local space. Guarantees gap-free coverage when rotation is involved.         | [pixel-bodies.md](physics/pixel-bodies.md)   |
| **Readback**           | Mapping CA simulation changes (destroyed/moved pixels) back to the pixel body's shape mask. Phase 2 of pixel bodies (not yet implemented).               | [pixel-bodies.md](physics/pixel-bodies.md)   |
| **Object Splitting**   | When destruction disconnects parts of a pixel body, connected component analysis detects multiple regions and spawns new entities for each fragment.      | [pixel-bodies.md](physics/pixel-bodies.md)   |
| **PixelBodyId**        | Stable u64 identifier persisting across save/load cycles. Generated from session seed + counter to prevent collisions.                                   | [pixel-bodies.md](physics/pixel-bodies.md)   |
| **Persistable**        | Marker component indicating a pixel body should be saved to disk when its chunk unloads and restored when the chunk loads again.                         | [pixel-bodies.md](physics/pixel-bodies.md)   |

---

## Collision

Terms for physics collision mesh generation. See [collision.md](physics/collision.md) for full architecture.

| Term                             | Definition                                                                                                                | Documentation                      |
|----------------------------------|---------------------------------------------------------------------------------------------------------------------------|------------------------------------|
| **Collision mesh**               | Generated geometry from solid pixels for physics interactions.                                                            | [collision.md](physics/collision.md)       |
| **Marching squares**             | Algorithm to extract contour polygons from binary (solid/non-solid) pixel grid. Used for both terrain and pixel bodies.  | [collision.md](physics/collision.md)       |
| **Douglas-Peucker**              | Line simplification algorithm that reduces vertex count while preserving shape. Applied to marching squares output.       | [collision.md](physics/collision.md)       |
| **Triangulation**                | Converting simplified polygon outlines into triangle meshes suitable for physics engines (Avian2D or Rapier2D).           | [collision.md](physics/collision.md)       |
| **Connected component analysis** | Algorithm detecting separate regions in a shape mask. Used to identify when a pixel body should split into fragments.     | [pixel-bodies.md](physics/pixel-bodies.md) |

### Collision Caching

| Term                    | Definition                                                                                                         | Documentation                |
|-------------------------|-------------------------------------------------------------------------------------------------------------------|------------------------------|
| **CollisionCache**      | Resource caching generated meshes by tile position. Tracks in-flight tasks and generation counters.               | [collision.md](physics/collision.md) |
| **In-flight**           | A tile with an active async mesh generation task. Prevents duplicate tasks; invalidation discards pending results.| [collision.md](physics/collision.md) |
| **Generation counter**  | Monotonic counter incremented on each cache insert. Colliders track their creation generation to detect staleness.| [collision.md](physics/collision.md) |
| **CollisionQueryPoint** | Marker component for entities that drive collision mesh generation in their proximity radius.                      | [collision.md](physics/collision.md) |

### Physics Integration

| Term              | Definition                                                                                                                  | Documentation                |
|-------------------|-----------------------------------------------------------------------------------------------------------------------------|------------------------------|
| **TileCollider**  | Static physics collider entity spawned from cached mesh. Tracks tile position and generation for staleness detection.       | [collision.md](physics/collision.md) |
| **Body wake**     | Sleeping physics bodies are woken when nearby terrain changes. Prevents bodies from floating after ground is removed.       | [collision.md](physics/collision.md) |

---

## Entity Culling

Automatic disabling of entities outside the streaming window. See [collision.md](physics/collision.md).

| Term              | Definition                                                                                                                            | Documentation                |
|-------------------|---------------------------------------------------------------------------------------------------------------------------------------|------------------------------|
| **StreamCulled**  | Marker component for entities that should be auto-disabled when outside the streaming window.                                          | [collision.md](physics/collision.md) |
| **CulledByWindow**| Internal marker distinguishing system-disabled entities from user-disabled. Only system-disabled entities are re-enabled on re-entry. | [collision.md](physics/collision.md) |
| **Collision ready**| A tile is cached and not in-flight. Culled entities wait for collision ready before re-enabling to prevent fall-through.             | [collision.md](physics/collision.md) |

---

## Thermal System

Terms for heat propagation and effects.

| Term                   | Definition                                                                  | Documentation                        |
|------------------------|-----------------------------------------------------------------------------|--------------------------------------|
| **Temperature**        | u8 value (0-255) in heat layer cells representing thermal energy.           | [simulation.md](simulation/simulation.md)       |
| **base_temperature**   | Material property: heat continuously emitted (lava=255, ice=0).             | [materials.md](simulation/materials.md)         |
| **ignition_threshold** | Material property: heat level required to catch fire. 0 = non-flammable.    | [materials.md](simulation/materials.md)         |
| **melting_threshold**  | Material property: heat level for state transition (stone->lava, ice->water). | [materials.md](simulation/materials.md)         |
| **cooling_factor**     | Configuration: heat dissipation rate per propagation pass (0.0-1.0).        | [configuration.md](foundational/configuration.md) |
| **burning_heat**       | Configuration: heat emitted by burning pixels per tick.                     | [configuration.md](foundational/configuration.md) |

---

## Related Documentation

- [Architecture Overview](README.md) - System architecture and design principles
- [Pixel Bodies](physics/pixel-bodies.md) - Dynamic physics objects with pixel content
- [Collision](physics/collision.md) - Physics collision mesh generation
- [Configuration Reference](foundational/configuration.md) - Tunable parameters
