# game

Example game demonstrating the `bevy_pixel_world` plugin.

## Running

```bash
# Release mode (recommended)
just run                    # or: cargo run -p game --release

# Development mode (dynamic linking for faster iteration)
just dev                    # or: cargo run -p game --features dev

# WASM development server
just serve                  # or: cd crates/game && trunk serve
```

## Controls

### Player

| Input | Action |
|-------|--------|
| A/D or Arrow Keys | Move |
| Space | Fly/Jump |
| F | Spawn physics body |

### Creative Mode (via `creative` command)

| Input | Action |
|-------|--------|
| LMB | Paint material |
| RMB | Erase |
| Scroll | Brush size |

### General

| Input | Action |
|-------|--------|
| / | Toggle console |
| Ctrl+S | Save world |

## Console

Press `/` to open the developer console. Available commands:

| Command | Description | Example |
|---------|-------------|---------|
| `tp <x> <y>` | Teleport player | `tp 0 500` |
| `time <value>` | Set time of day | `time 6am`, `time 18`, `time 14:30` |
| `spawn <object>` | Spawn object above player | `spawn bomb`, `spawn box`, `spawn femur` |
| `creative` | Toggle creative mode | `creative` |

## Editor Mode

The level editor requires the `editor` feature (native only, not supported on WASM).

```bash
cargo run -p game --features editor --release
```

### Editor Controls

| Input | Action |
|-------|--------|
| WASD | Pan camera |
| F5 | Enter play mode |
| Escape | Return to editor |

Levels are stored in `assets/levels/` as `.yol` files (Yoleck format).

## Feature Flags

| Feature | Description |
|---------|-------------|
| `dev` | Dynamic linking + file watcher + editor |
| `editor` | Level editor (Yoleck) - native only |

## Configuration

Game configuration is loaded from `assets/config/game.config.toml`.

Material definitions are in `assets/config/materials.toml`.
