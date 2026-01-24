# Configuration Reference

Parameters for the pixel sandbox plugin. **Architectural constants are compile-time values defined in code, not runtime
configuration.**

> **Note:** The specific values in this document are illustrative examples. Actual compile-time values are defined in
> `crates/pixel_world/src/coords.rs` and may differ. Consult the source code for authoritative values.

## Compile-Time Constants

These values are hardcoded and never passed through function arguments.

### Primary Constants

| Constant        | Value | Description                                |
|-----------------|-------|--------------------------------------------|
| `CHUNK_SIZE`    | 512   | Pixels per chunk edge (square chunks)      |
| `TILE_SIZE`     | 16    | Pixels per tile edge in checkerboard       |
| `WINDOW_WIDTH`  | 6     | Chunks loaded horizontally                 |
| `WINDOW_HEIGHT` | 4     | Chunks loaded vertically                   |
| `PHASES`        | 4     | Checkerboard phases (fixed for 2×2 tiles)  |

The window dimensions (6×4) are sized for landscape orientation, covering the typical viewport without overshooting.
The rolling grid maintains a fixed rectangular shape—chunks roll from one edge to the opposite as the camera moves,
preserving internal positional consistency.

### Derived Constants

Derived values are expressed as formulas, not magic numbers:

| Constant          | Formula                         | Value     |
|-------------------|---------------------------------|-----------|
| `POOL_SIZE`       | `WINDOW_WIDTH * WINDOW_HEIGHT`  | 24 chunks |
| `TILES_PER_CHUNK` | `CHUNK_SIZE / TILE_SIZE`        | 32 tiles  |
| `CHUNK_MEMORY`    | `CHUNK_SIZE * CHUNK_SIZE * 4`   | 1 MB      |

**Constraint:** `CHUNK_SIZE` must be evenly divisible by `TILE_SIZE`. This ensures the checkerboard pattern aligns
across chunk boundaries. See [Simulation](simulation.md) for details.

## Runtime Parameters

These may vary per session or be user-configurable:

| Parameter           | Description                                | Default |
|---------------------|--------------------------------------------|---------|
| `world_seed`        | Base seed for deterministic generation     | random  |
| `hysteresis_frames` | Frames of stable movement before recycling | 5       |

## Tick Rates

All simulation passes run on fixed update loops at specific ticks per second (TPS):

| Pass                  | TPS | Description                                             |
|-----------------------|-----|---------------------------------------------------------|
| Cellular Automata     | 60  | Physics movement, material interactions                 |
| Particles             | 60  | Free-form particle updates (runs with CA)               |
| Collision mesh update | 60  | Regenerates when CA changes or physics objects approach |
| Decay                 | 20  | Time-based material transformations                     |
| Heat propagation      | 10  | Thermal diffusion across heat layer                     |

**Note:** Interaction `rate` values in material definitions are effect applications per interaction tick (at CA TPS).

## Chunk Seeding

### Noise Seeder

| Parameter         | Description                            | Constraints                      |
|-------------------|----------------------------------------|----------------------------------|
| `world_seed`      | Base seed for deterministic generation | Any integer                      |
| `noise_type`      | Algorithm for terrain generation       | Perlin, Simplex, Cellular, Value |
| `noise_frequency` | Scale of terrain features              | Lower = larger features          |
| `noise_octaves`   | Layers of detail                       | More = finer detail, slower      |

### Persistence Seeder

| Parameter     | Description                     | Constraints                    |
|---------------|---------------------------------|--------------------------------|
| `compression` | Algorithm for chunk storage     | LZ4 recommended for speed      |
| `save_path`   | Directory for chunk files       | Must be writable               |
| `async_io`    | Whether to use non-blocking I/O | Recommended for responsiveness |

## Decay Pass

| Parameter   | Description                 | Constraints     |
|-------------|-----------------------------|-----------------|
| `decay_tps` | Decay pass ticks per second | Default: 20 TPS |

**Note:** Material `decay_chance` values are calibrated assuming this tick rate. Adjusting the rate affects how quickly
materials decay in real time.

## Heat Propagation

| Parameter        | Description                                | Constraints         |
|------------------|--------------------------------------------|---------------------|
| `cooling_factor` | Heat dissipation rate per propagation pass | 0.0-1.0, e.g., 0.95 |
| `burning_heat`   | Heat emitted by burning pixels per tick    | e.g., 50            |

**Note:** Higher `cooling_factor` values mean heat persists longer. See [Simulation](simulation.md) for heat layer
details.

## Particle Physics

| Parameter   | Description                                   | Constraints        |
|-------------|-----------------------------------------------|--------------------|
| `air_drag`  | Velocity damping coefficient per tick         | 0.0-1.0, e.g., 0.1 |
| `pool_size` | Maximum concurrent particles before rejecting | e.g., 10000        |

**Note:** See [Particles](particles.md) for particle system documentation.

## Memory Budget

With the default constants:

- Chunk memory: 512 × 512 × 4 = 1 MB per chunk
- Total pool: 24 × 1 MB = 24 MB
- World coverage: (6 × 512) × (4 × 512) = 3072 × 2048 pixels

## Related Documentation

- [Pixel Format](pixel-format.md) - Defines bytes_per_pixel structure
- [Chunk Pooling](chunk-pooling.md) - How pool parameters affect memory
- [Streaming Window](streaming-window.md) - How window parameters affect loading
- [Simulation](simulation.md) - How tile size affects parallelism
- [Chunk Seeding](chunk-seeding.md) - How seeder parameters affect generation
- [Architecture Overview](README.md)
