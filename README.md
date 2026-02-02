<p align="center">
  <img src=".github/images/warn.png" alt="Warning: Experimental Project" width="100%"/>
</p>

# sim2d

An experiment in **spec-driven AI development** of a complex game engine.

This project explores what happens when you give Claude detailed specifications and let it build a Noita-style falling sand engine from scratch. The goal is to understand both the capabilities and failure modes of AI-assisted development on non-trivial systems.

## The Experiment

Every component of this engine was designed through specification documents and implemented by Claude. Human involvement is limited to:

- Writing specifications that describe *what* to build
- Reviewing and approving implementation plans
- Catching AI mistakes before they compound
- Documenting failure patterns for future reference

The codebase serves as both a functional game engine and a record of AI development patterns—both successful and disastrous.

## What Got Built

A Bevy plugin that handles the hard parts of pixel simulation games:

- **Infinite streaming worlds** that load chunks around the camera
- **Cellular automata simulation** with four aggregate states: solid, powder, liquid, gas
- **Data-driven materials** - define physics and interactions in TOML
- **Automatic collision meshes** generated from pixel data via marching squares
- **Destructible pixel bodies** - rigid bodies made of pixels that take damage
- **Full persistence** - save and load worlds on native and WASM (via OPFS)

Physics backends: [Avian2D](https://github.com/Jondolf/avian) or [Rapier2D](https://rapier.rs/).

## AI Failure Log

See [`docs/llm-cases/`](docs/llm-cases/) for documented cases where Claude made decisions that hurt the codebase.

These aren't fixable through better prompting—they're artifacts of how LLMs work. The goal is to accumulate enough concrete examples to inform a better methodology for spec-driven LLM development.

## Quick Start

```bash
# Run the example game
just run                    # or: cargo run -p game --release

# Development mode (dynamic linking)
just dev                    # or: cargo run -p game --features dev

# Run tests
just test                   # or: cargo test -p bevy_pixel_world

# WASM development server
just serve                  # or: cd crates/game && trunk serve

# Build NoiseTool (required for noise profile editing in level editor)
just build-noise-tool
```

### Controls

| Input | Action |
|-------|--------|
| LMB | Paint |
| RMB | Erase |
| Scroll | Brush size |
| Ctrl+S | Save |
| / | Toggle console |

### Console Commands

| Command | Description |
|---------|-------------|
| `tp <x> <y>` | Teleport player to coordinates |
| `time <value>` | Set time of day (e.g. `6am`, `18`, `14:30`) |
| `spawn <object>` | Spawn object above player (`bomb`, `box`, `femur`) |
| `creative` | Toggle creative mode (paint/erase pixels) |

## Core Functionality

- Infinite streaming chunks around camera
- Cellular automata: solid, powder, liquid, gas
- Data-driven materials (TOML)
- Marching squares collision meshes
- Destructible pixel bodies with physics
- Full persistence (native/WASM OPFS)

## Public API Reference

### Plugin Setup

#### `PixelWorldPlugin::new(persistence: PersistenceConfig) -> Self`
Core plugin for infinite cellular automata simulation.

```rust
app.add_plugins(PixelWorldPlugin::new(
    PersistenceConfig::at("world.save").with_seed(42)
));
```

#### `PixelWorldFullBundle::new(persistence: PersistenceConfig) -> Self`
Convenience bundle adding all sub-plugins (bodies, buoyancy, diagnostics).

```rust
app.add_plugins(
    PixelWorldFullBundle::new(PersistenceConfig::at("world.save"))
        .submersion(SubmersionConfig { threshold: 0.5, ..default() })
        .buoyancy(BuoyancyConfig::default())
);
```

---

### World Spawning

#### `SpawnPixelWorld::new(seeder: impl ChunkSeeder) -> Self`
Command to spawn a pixel world with a chunk seeder.

```rust
commands.spawn(SpawnPixelWorld::new(MaterialSeeder::new(42)));
```

#### `StreamingCamera` (Component)
Marker component for cameras that drive chunk streaming.

```rust
commands.spawn((Camera2d, StreamingCamera));
```

---

### Pixel Access

#### `PixelWorld::get_pixel(&self, pos: WorldPos) -> Option<&Pixel>`
Returns pixel at world position. `None` if chunk not loaded/seeded.

#### `PixelWorld::set_pixel(&mut self, pos: WorldPos, pixel: Pixel, gizmos) -> bool`
Sets pixel at world position. Returns `true` if successful.

```rust
fn paint_system(mut worlds: Query<&mut PixelWorld>) {
    let mut world = worlds.single_mut();
    let pos = WorldPos::new(100, 200);
    let pixel = Pixel::new(material_ids::SAND, ColorIndex(128));
    world.set_pixel(pos, pixel, ());
}
```

#### `PixelWorld::swap_pixels(&mut self, a: WorldPos, b: WorldPos) -> bool`
Swaps two pixels atomically. Works across chunk boundaries.

#### `PixelWorld::get_heat_at(&self, pos: WorldPos) -> Option<u8>`
Returns heat value (0-255) at position's heat cell.

#### `PixelWorld::set_heat_at(&mut self, pos: WorldPos, heat: u8) -> bool`
Sets heat value at position's heat cell.

---

### Pixel Bodies

#### `SpawnPixelBody::new(path, material, position) -> Self`
Command to spawn a destructible physics body from an image.

| Parameter | Type | Description |
|-----------|------|-------------|
| `path` | `impl Into<String>` | Asset path relative to `assets/` |
| `material` | `MaterialId` | Material for all pixels |
| `position` | `Vec2` | World spawn position |

```rust
commands.queue(SpawnPixelBody::new(
    "sprites/crate.png",
    material_ids::WOOD,
    Vec2::new(100.0, 200.0),
));
```

#### `SpawnPixelBody::with_extra<F>(self, f: F) -> Self`
Adds extra components to the spawned entity.

```rust
commands.queue(
    SpawnPixelBody::new("box.png", material_ids::WOOD, pos)
        .with_extra(|entity| {
            entity.insert(Bomb {
                damage_threshold: 0.03,
                blast_radius: 120.0,
                blast_strength: 60.0,
                detonated: false,
            });
        })
);
```

#### `PixelBody` (Component)
Destructible physics object. Key methods:

| Method | Returns | Description |
|--------|---------|-------------|
| `width()` | `u32` | Pixel grid width |
| `height()` | `u32` | Pixel grid height |
| `is_solid(x, y)` | `bool` | Whether pixel belongs to body |
| `get_pixel(x, y)` | `Option<&Pixel>` | Pixel at local coords |
| `solid_count()` | `usize` | Number of solid pixels |
| `is_empty()` | `bool` | True if fully destroyed |

---

### Blasts & Explosions

#### `PixelWorld::blast(&mut self, params: &BlastParams, callback)`
Radial ray-cast explosion from center point.

```rust
world.blast(&BlastParams {
    center: Vec2::new(100.0, 200.0),
    strength: 60.0,
    max_radius: 120.0,
    heat_radius: 80.0,
}, |pixel, pos| {
    if pixel.material() == material_ids::STONE {
        BlastHit::Hit { pixel: Pixel::void(), cost: 2.0 }
    } else {
        BlastHit::Hit { pixel: Pixel::void(), cost: 1.0 }
    }
});
```

#### `BlastHit` (enum)
Callback return value controlling ray behavior:

| Variant | Effect |
|---------|--------|
| `Skip` | Continue ray, no energy cost |
| `Hit { pixel, cost }` | Replace pixel, consume energy |
| `Stop` | Terminate ray immediately |

---

### Chunk Seeding

#### `MaterialSeeder::new(seed: i32) -> Self`
Procedural terrain seeder with noise-based material placement.

```rust
let seeder = MaterialSeeder::new(42);
commands.spawn(SpawnPixelWorld::new(seeder));
```

#### `ChunkSeeder` (trait)
Implement to create custom procedural generation:

```rust
impl ChunkSeeder for MySeeder {
    fn seed(&self, pos: ChunkPos, chunk: &mut Chunk) {
        for y in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                chunk.pixels.set(x, y, Pixel::new(material_ids::STONE, ColorIndex(128)));
            }
        }
    }
}
```

---

### World State

#### `WorldInitState` (Resource)
Tracks initialization progress:

| State | Description |
|-------|-------------|
| `Initializing` | Reading save file index |
| `LoadingChunks` | Initial chunks loading/seeding |
| `Ready` | Gameplay can begin |

#### `world_is_ready(state: Res<WorldInitState>) -> bool`
Run condition for gameplay systems.

```rust
app.add_systems(Update, player_movement.run_if(world_is_ready));
```

#### `WorldLoadingProgress` (Resource)
Loading screen metrics:

| Field | Type | Description |
|-------|------|-------------|
| `chunks_ready` | `usize` | Loaded chunk count |
| `chunks_total` | `usize` | Total chunks needed |
| `fraction()` | `f32` | Progress 0.0-1.0 |

---

### Persistence

#### `PersistenceControl::save(&mut self) -> PersistenceHandle`
Triggers a manual save operation.

```rust
fn save_hotkey(mut persistence: ResMut<PersistenceControl>, keys: Res<ButtonInput<KeyCode>>) {
    if keys.just_pressed(KeyCode::F5) {
        persistence.save();
    }
}
```

#### `SimulationState` (Resource)
Pause/resume simulation:

```rust
simulation_state.paused = true;  // Freeze CA simulation
```

---

### Coordinates

| Type | Description |
|------|-------------|
| `WorldPos` | Absolute pixel position (i64, i64) |
| `ChunkPos` | Chunk index (i32, i32) |
| `LocalPos` | Pixel within chunk (0..128, 0..128) |
| `WorldRect` | AABB with x, y, width, height |

```rust
let world_pos = WorldPos::new(1000, 2000);
let (chunk, local) = world_pos.to_chunk_and_local();
```

---

### Materials

Materials defined in TOML (`assets/config/materials.toml`):

```toml
[[materials]]
name = "sand"
physics_state = "powder"
density = 1.5
friction = 0.3
blast_resistance = 0.5
colors = [[194, 178, 128], [189, 174, 124]]
```

Access via `material_ids`:

```rust
use bevy_pixel_world::material_ids;

let sand = Pixel::new(material_ids::SAND, ColorIndex(0));
let water = Pixel::new(material_ids::WATER, ColorIndex(0));
```

## Project Structure

```
crates/
├── bevy_pixel_world/   # Core plugin
├── game/               # Example game
└── sim2d_noise/        # Noise utilities (WASM)

docs/
├── architecture/       # How things work internally
└── implementation/     # Development methodology
```

## License

This repository contains code under multiple licenses:

| Path | License |
|------|---------|
| `crates/bevy_pixel_world/` | MIT |
| `crates/game/` | MIT |
| `crates/noise_ipc/` | MIT |
| `crates/sim2d_noise/` | MIT |
| `assets/` | MIT |
| `assets/sprites/cc0/` | CC0 ([OpenGameArt](https://opengameart.org)) |
| `docs/` | MIT |
| `scripts/` | MIT |
| `workers/` | MIT |
| `vendor/bevy_crt/` | GPL-3.0-or-later |

The CRT shader code in `vendor/bevy_crt/` is derived from guest.r's [crt-guest-advanced-hd](https://github.com/libretro/slang-shaders) shaders (GPL-3.0-or-later). This component is isolated in `vendor/` and does not affect the licensing of the rest of the codebase.
