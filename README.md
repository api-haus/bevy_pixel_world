# bevy_pixel_world

A 2D pixel sandbox simulation plugin for [Bevy](https://bevyengine.org/). Features infinite worlds with cellular automata physics.

## Features

- **Infinite worlds** - Streaming window loads chunks around the camera on demand
- **Cellular automata** - Falling sand physics: powder falls, liquid flows
- **Zero-allocation pooling** - All chunk memory is pre-allocated and reused
- **Parallel simulation** - Checkerboard scheduling enables safe concurrent updates
- **Persistence** - Chunks saved to disk with LZ4 compression
- **Procedural generation** - Terrain generated with FastNoise2

## Quick Start

```bash
# Run the painting demo
cargo run -p bevy_pixel_world --example painting

# Release mode
cargo run -p bevy_pixel_world --example painting --release --no-default-features --features diagnostics
```

### Painting Demo Controls

| Input | Action |
|-------|--------|
| LMB | Paint with selected material |
| RMB | Erase |
| Scroll | Adjust brush size |
| WASD | Move camera |
| Shift | Speed boost |

## Project Structure

```
crates/
├── bevy_pixel_world/   # Core simulation plugin (publishable)
└── game/               # Example game application
```

## Requirements

- Rust 2024 edition
- Bevy 0.17

## Documentation

See [docs/arhitecture/](docs/arhitecture/README.md) for architecture documentation.

## License

MIT License - see [LICENSE](LICENSE)
