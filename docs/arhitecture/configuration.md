# Configuration Reference

Tunable parameters for the pixel sandbox plugin.

## Chunk Pool

| Parameter | Description | Constraints |
|-----------|-------------|-------------|
| `pool_size` | Number of chunks in the object pool | Must match active region dimensions |
| `chunk_width` | Horizontal pixels per chunk | Power of 2 recommended |
| `chunk_height` | Vertical pixels per chunk | Power of 2 recommended |
| `bytes_per_pixel` | Storage per pixel | Fixed at 4 bytes per [Pixel Format](pixel-format.md) |

**Derived values:**
- `chunk_memory` = `chunk_width` × `chunk_height` × `bytes_per_pixel`
- `total_pool_memory` = `pool_size` × `chunk_memory`

## Streaming Window

| Parameter | Description | Constraints |
|-----------|-------------|-------------|
| `window_width` | Chunks loaded horizontally | Determines horizontal view distance |
| `window_height` | Chunks loaded vertically | Determines vertical view distance |
| `hysteresis_frames` | Frames of stable movement before recycling | Higher = more stable, slower response |

**Derived values:**
- `active_region_size` = `window_width` × `window_height` (must equal `pool_size`)
- `world_coverage` = (`window_width` × `chunk_width`) × (`window_height` × `chunk_height`)

## Simulation

| Parameter | Description | Constraints |
|-----------|-------------|-------------|
| `tile_size` | Pixels per tile edge in checkerboard | Affects parallelism granularity |
| `phases_per_tick` | Number of checkerboard phases | Fixed at 4 for 2×2 tile pattern |

## Chunk Seeding

### Noise Seeder

| Parameter | Description | Constraints |
|-----------|-------------|-------------|
| `world_seed` | Base seed for deterministic generation | Any integer |
| `noise_type` | Algorithm for terrain generation | Perlin, Simplex, Cellular, Value |
| `noise_frequency` | Scale of terrain features | Lower = larger features |
| `noise_octaves` | Layers of detail | More = finer detail, slower |

### Persistence Seeder

| Parameter | Description | Constraints |
|-----------|-------------|-------------|
| `compression` | Algorithm for chunk storage | LZ4 recommended for speed |
| `save_path` | Directory for chunk files | Must be writable |
| `async_io` | Whether to use non-blocking I/O | Recommended for responsiveness |

## Example Configuration

A typical configuration for reference (actual values depend on target platform and requirements):

```
Chunk Pool:
  pool_size: <active region total>
  chunk_dimensions: <power of 2 square>
  bytes_per_pixel: <4 for RGBA or type+metadata>

Streaming Window:
  window_dimensions: <based on view distance needs>
  hysteresis_frames: <tune for responsiveness vs stability>

Simulation:
  tile_size: <2 for standard checkerboard>
  phases_per_tick: <4 for 2×2 tiles>
```

## Related Documentation

- [Pixel Format](pixel-format.md) - Defines bytes_per_pixel structure
- [Chunk Pooling](chunk-pooling.md) - How pool parameters affect memory
- [Streaming Window](streaming-window.md) - How window parameters affect loading
- [Simulation](simulation.md) - How tile size affects parallelism
- [Chunk Seeding](chunk-seeding.md) - How seeder parameters affect generation
- [Architecture Overview](README.md)
