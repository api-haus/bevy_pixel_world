# Configuration Reference

Tunable parameters for the pixel sandbox plugin.

## Chunk Pool

| Parameter         | Description                         | Constraints                                          |
|-------------------|-------------------------------------|------------------------------------------------------|
| `pool_size`       | Number of chunks in the object pool | Must match active region dimensions                  |
| `chunk_width`     | Horizontal pixels per chunk         | Power of 2 recommended                               |
| `chunk_height`    | Vertical pixels per chunk           | Power of 2 recommended                               |
| `bytes_per_pixel` | Storage per pixel                   | Fixed at 4 bytes per [Pixel Format](pixel-format.md) |

**Derived values:**

- `chunk_memory` = `chunk_width` × `chunk_height` × `bytes_per_pixel`
- `total_pool_memory` = `pool_size` × `chunk_memory`

## Streaming Window

| Parameter           | Description                                | Constraints                           |
|---------------------|--------------------------------------------|---------------------------------------|
| `window_width`      | Chunks loaded horizontally                 | Determines horizontal view distance   |
| `window_height`     | Chunks loaded vertically                   | Determines vertical view distance     |
| `hysteresis_frames` | Frames of stable movement before recycling | Higher = more stable, slower response |

**Derived values:**

- `active_region_size` = `window_width` × `window_height` (must equal `pool_size`)
- `world_coverage` = (`window_width` × `chunk_width`) × (`window_height` × `chunk_height`)

## Simulation

| Parameter         | Description                          | Constraints                     |
|-------------------|--------------------------------------|---------------------------------|
| `tile_size`       | Pixels per tile edge in checkerboard | Affects parallelism granularity |
| `phases_per_tick` | Number of checkerboard phases        | Fixed at 4 for 2×2 tile pattern |

**Constraint:** Chunk dimensions must be evenly divisible by `tile_size` (i.e., chunks must contain an even number of
tiles in each dimension). This ensures the checkerboard pattern aligns across chunk boundaries.
See [Simulation](simulation.md) for details.

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

## Example Configuration

A typical configuration for reference (actual values depend on target platform and requirements):

```
Chunk Pool:
  pool_size: 81              # 9×9 window
  chunk_width: 512
  chunk_height: 512
  bytes_per_pixel: 4

Streaming Window:
  window_width: 9            # chunks
  window_height: 9           # chunks
  hysteresis_frames: 5

Simulation:
  tile_size: 16              # 16×16 pixels per tile
  phases_per_tick: 4
  ca_tps: 60                 # cellular automata ticks per second
  decay_tps: 20              # decay pass ticks per second
  heat_tps: 10               # heat propagation ticks per second

Heat:
  cooling_factor: 0.95
  burning_heat: 50

Particles:
  air_drag: 0.1
  pool_size: 10000
```

**Memory calculation for this example:**

- Chunk memory: 512 × 512 × 4 = 1 MB per chunk
- Total pool: 81 × 1 MB = 81 MB
- World coverage: (9 × 512) × (9 × 512) = 4608 × 4608 pixels

## Related Documentation

- [Pixel Format](pixel-format.md) - Defines bytes_per_pixel structure
- [Chunk Pooling](chunk-pooling.md) - How pool parameters affect memory
- [Streaming Window](streaming-window.md) - How window parameters affect loading
- [Simulation](simulation.md) - How tile size affects parallelism
- [Chunk Seeding](chunk-seeding.md) - How seeder parameters affect generation
- [Architecture Overview](README.md)
