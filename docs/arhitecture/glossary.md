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
| **World** | Infinite 2D coordinate space providing global addressing. Has no direct memory representation. | [spatial-hierarchy.md](spatial-hierarchy.md) |
| **Chunk** | Fixed-size rectangular pixel buffer. Unit of pooling, streaming, persistence, and rendering.   | [spatial-hierarchy.md](spatial-hierarchy.md) |
| **Tile**  | Subdivision of a chunk used for checkerboard scheduling and dirty rect tracking.               | [spatial-hierarchy.md](spatial-hierarchy.md) |
| **Pixel** | Fundamental 4-byte simulation unit containing material, color, damage, and flags.              | [pixel-format.md](pixel-format.md)           |

### Coordinate Systems

| Term                  | Range                  | Usage                                      |
|-----------------------|------------------------|--------------------------------------------|
| **World coordinates** | Infinite (signed)      | Global pixel addressing                    |
| **Chunk coordinates** | Infinite (signed)      | Identifies which chunk contains a position |
| **Tile coordinates**  | 0 to tiles_per_chunk-1 | Position within chunk's tile grid          |
| **Local coordinates** | 0 to chunk_size-1      | Pixel position within a chunk              |

---

## Material System

Terms related to material definitions and behavior.

| Term                  | Definition                                                                                                          | Documentation                |
|-----------------------|---------------------------------------------------------------------------------------------------------------------|------------------------------|
| **Material**          | Type definition controlling pixel behavior. Indexed by u8 ID into a registry of up to 256 materials.                | [materials.md](materials.md) |
| **Material Registry** | Array storing all material definitions, indexed by Material ID.                                                     | [materials.md](materials.md) |
| **void**              | Reserved Material ID 0 representing empty space.                                                                    | [materials.md](materials.md) |
| **Tag**               | Category label for interaction targeting (e.g., `stone`, `organic`, `flammable`). Materials can have multiple tags. | [materials.md](materials.md) |
| **Interaction**       | Definition of what happens when materials contact: corrode, ignite, diffuse, transform, displace, or none.          | [materials.md](materials.md) |

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
themselves. See [materials.md](materials.md) for the full convention.

---

## Pixel Data

Fields comprising the 4-byte pixel structure.

| Term               | Type | Definition                                                            | Documentation                      |
|--------------------|------|-----------------------------------------------------------------------|------------------------------------|
| **Material field** | u8   | Type identifier indexing into material registry.                      | [pixel-format.md](pixel-format.md) |
| **Color field**    | u8   | Palette index for rendering; allows per-pixel visual variation.       | [pixel-format.md](pixel-format.md) |
| **Damage field**   | u8   | Accumulated damage; triggers destruction/transformation at threshold. | [pixel-format.md](pixel-format.md) |
| **Flags field**    | u8   | Packed boolean states for simulation and rendering.                   | [pixel-format.md](pixel-format.md) |

### Pixel Flags

| Flag        | Bit | Definition                                                                                             |
|-------------|-----|--------------------------------------------------------------------------------------------------------|
| **dirty**   | 0   | Pixel needs simulation this tick. Stable pixels have `dirty=0` and are skipped.                        |
| **solid**   | 1   | Cached check: material state is `solid` or `powder` (not `liquid` or `gas`). Used by collision system. |
| **falling** | 2   | Pixel has downward momentum. Excluded from collision mesh while set.                                   |
| **burning** | 3   | Pixel is on fire. Propagates to flammable neighbors; increments damage.                                |
| **wet**     | 4   | Pixel is saturated with liquid. Prevents ignition; modifies behavior.                                  |

---

## Simulation

Terms related to the multi-pass simulation system.

| Term                        | Definition                                                                                          | Documentation                                |
|-----------------------------|-----------------------------------------------------------------------------------------------------|----------------------------------------------|
| **Cellular Automata (CA)**  | Grid-based simulation where pixels update based on neighbor rules and material state.               | [simulation.md](simulation.md)               |
| **Falling sand**            | Genre/technique for powder and liquid physics via cellular automata.                                | [simulation.md](simulation.md)               |
| **Tick**                    | One complete simulation cycle comprising all passes.                                                | [simulation.md](simulation.md)               |
| **TPS**                     | Ticks per second. Fixed update rates: CA/Particles/Collision=60, Decay=20, Heat=10.                 | [configuration.md](configuration.md)         |
| **Pass**                    | Single processing sweep: CA pass, particle pass, material interactions pass, decay pass, heat pass. | [simulation.md](simulation.md)               |
| **Checkerboard scheduling** | 2x2 phase pattern (A, B, C, D) enabling parallel tile processing without race conditions.           | [simulation.md](simulation.md)               |
| **Phase**                   | One of four checkerboard groups. Tiles of the same phase process in parallel.                       | [simulation.md](simulation.md)               |
| **Dirty rect**              | Per-tile bounding box for simulation scheduling. Distinct from per-pixel dirty flag.                | [spatial-hierarchy.md](spatial-hierarchy.md) |

### Simulation Layers

| Term           | Definition                                                                                         | Documentation                  |
|----------------|----------------------------------------------------------------------------------------------------|--------------------------------|
| **Heat layer** | Downsampled thermal map (chunk_size/4 resolution). Stores u8 temperature values (0=cold, 255=hot). | [simulation.md](simulation.md) |
| **Decay**      | Probabilistic time-based material transformation independent of pixel activity.                    | [simulation.md](simulation.md) |

---

## Particle System

Terms for the free-form particle effects system.

| Term                 | Definition                                                                                          | Documentation                |
|----------------------|-----------------------------------------------------------------------------------------------------|------------------------------|
| **Particle**         | Free-form entity with sub-pixel position and velocity for dynamic effects (debris, pouring, gases). | [particles.md](particles.md) |
| **Emission**         | Pixel to particle transition triggered by explosion, pouring, or gas release.                       | [particles.md](particles.md) |
| **Deposition**       | Particle to pixel transition when settling or colliding with the grid.                              | [particles.md](particles.md) |
| **particle_gravity** | Material property controlling particle fall rate. Negative values cause rising (steam, smoke).      | [materials.md](materials.md) |

---

## Streaming & Memory

Terms for chunk lifecycle and memory management.

| Term                  | Definition                                                                                       | Documentation                              |
|-----------------------|--------------------------------------------------------------------------------------------------|--------------------------------------------|
| **Chunk Pool**        | Object pool of pre-allocated chunk buffers enabling zero runtime allocation.                     | [chunk-pooling.md](chunk-pooling.md)       |
| **Streaming window**  | Camera-tracking region that determines which chunks are loaded. Synonymous with "active region". | [streaming-window.md](streaming-window.md) |
| **Active region**     | Set of currently loaded chunks around the camera. Synonymous with "streaming window".            | [streaming-window.md](streaming-window.md) |
| **Seeding**           | Filling a chunk buffer with initial pixel data from noise or disk.                               | [chunk-seeding.md](chunk-seeding.md)       |
| **ChunkSeeder**       | Trait abstraction for populating chunks (noise seeder, persistence seeder).                      | [chunk-seeding.md](chunk-seeding.md)       |
| **Recycling**         | Returning a chunk to the pool when the camera moves away; optionally persists to disk.           | [chunk-pooling.md](chunk-pooling.md)       |
| **Hysteresis**        | Buffer preventing rapid chunk recycling when camera oscillates near boundaries.                  | [streaming-window.md](streaming-window.md) |
| **Delta persistence** | Optimization storing only differences from procedural generation instead of full buffers.        | [chunk-seeding.md](chunk-seeding.md)       |

---

## Procedural Generation

Terms for world generation systems.

| Term        | Definition                                                                           | Documentation                        |
|-------------|--------------------------------------------------------------------------------------|--------------------------------------|
| **PCG**     | Procedural Content Generation - algorithmic creation of world content.               | [pcg-ideas.md](pcg-ideas.md)         |
| **Noise**   | Coherent random functions (Perlin, Simplex, Cellular, Value) for terrain generation. | [chunk-seeding.md](chunk-seeding.md) |
| **WFC**     | Wave Function Collapse - constraint-based generation for macro-level structure.      | [pcg-ideas.md](pcg-ideas.md)         |
| **Stamp**   | Preset formation (cave, tree, building) placed during generation via stencil masks.  | [pcg-ideas.md](pcg-ideas.md)         |
| **Stencil** | Shape mask defining where a stamp applies to the world.                              | [pcg-ideas.md](pcg-ideas.md)         |

---

## Rendering

Terms for the rendering pipeline.

| Term                 | Definition                                                                     | Documentation                      |
|----------------------|--------------------------------------------------------------------------------|------------------------------------|
| **Chunk texture**    | GPU texture uploaded from chunk pixel buffer for rendering.                    | [rendering.md](rendering.md)       |
| **Palette**          | Color lookup table (up to 256 colors) indexed by pixel Color field.            | [pixel-format.md](pixel-format.md) |
| **Identity texture** | PNG asset defining repeating visual pattern applied during chunk seeding.      | [rendering.md](rendering.md)       |
| **Heat glow**        | Visual tinting of hot non-flammable materials based on heat layer temperature. | [simulation.md](simulation.md)     |

---

## Collision

Terms for physics collision mesh generation.

| Term                 | Definition                                                                      | Documentation                |
|----------------------|---------------------------------------------------------------------------------|------------------------------|
| **Collision mesh**   | Generated geometry from solid pixels for physics interactions.                  | [collision.md](collision.md) |
| **Marching squares** | Algorithm to extract contour polygons from binary (solid/non-solid) pixel grid. | [collision.md](collision.md) |

---

## Thermal System

Terms for heat propagation and effects.

| Term                   | Definition                                                                  | Documentation                        |
|------------------------|-----------------------------------------------------------------------------|--------------------------------------|
| **Temperature**        | u8 value (0-255) in heat layer cells representing thermal energy.           | [simulation.md](simulation.md)       |
| **base_temperature**   | Material property: heat continuously emitted (lava=255, ice=0).             | [materials.md](materials.md)         |
| **ignition_threshold** | Material property: heat level required to catch fire. 0 = non-flammable.    | [materials.md](materials.md)         |
| **melting_threshold**  | Material property: heat level for state transition (stone→lava, ice→water). | [materials.md](materials.md)         |
| **cooling_factor**     | Configuration: heat dissipation rate per propagation pass (0.0-1.0).        | [configuration.md](configuration.md) |
| **burning_heat**       | Configuration: heat emitted by burning pixels per tick.                     | [configuration.md](configuration.md) |

---

## Related Documentation

- [Architecture Overview](README.md) - System architecture and design principles
- [Configuration Reference](configuration.md) - Tunable parameters
