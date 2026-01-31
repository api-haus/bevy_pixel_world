# bevy_pixel_world

Infinite cellular automata simulation plugin for Bevy.

## Overview

A Bevy plugin for creating Noita-style falling sand simulations with:

- Infinite streaming chunks around camera
- Four aggregate states: solid, powder, liquid, gas
- Data-driven materials via TOML
- Automatic marching squares collision meshes
- Destructible pixel bodies with physics
- Full persistence (native filesystem / WASM OPFS)

## Usage

```rust
use bevy::prelude::*;
use bevy_pixel_world::{
    PersistenceConfig, PixelWorldPlugin, StreamingCamera, SpawnPixelWorld,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(PixelWorldPlugin::new(
            PersistenceConfig::at("world.save").with_seed(42)
        ))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    // Spawn camera with streaming enabled
    commands.spawn((Camera2d, StreamingCamera));

    // Spawn the pixel world
    commands.spawn(SpawnPixelWorld);
}
```

## Feature Flags

| Feature | Description |
|---------|-------------|
| `native` | Native filesystem support (default) |
| `avian2d` | Avian2D physics backend |
| `rapier2d` | Rapier2D physics backend |
| `tracy` | Tracy profiler integration |
| `dev` | Dynamic linking for faster iteration |

Enable a physics backend to use pixel bodies:

```toml
[dependencies]
bevy_pixel_world = { version = "0.1", features = ["avian2d"] }
```

## Tests

```bash
cargo test -p bevy_pixel_world
```

## Full API

See the [root README](../../README.md) for complete API reference.
