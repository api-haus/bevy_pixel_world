<p align="center">
  <img src=".github/images/warn.png" alt="Warning: Experimental Project" width="400"/>
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
# Run the painting demo
cargo run -p bevy_pixel_world --example painting

# Release mode
cargo run -p bevy_pixel_world --example painting --release
```

### Controls

| Input | Action |
|-------|--------|
| LMB | Paint |
| RMB | Erase |
| Scroll | Brush size |
| WASD | Move camera |
| Space | Spawn physics body |
| Ctrl+S | Save |

## Project Structure

```
crates/
├── bevy_pixel_world/   # Core plugin
├── game/               # Example game
└── sim2d_noise/        # Noise utilities

docs/
├── architecture/       # How things work internally
└── implementation/     # Development methodology
```

## License

MIT
