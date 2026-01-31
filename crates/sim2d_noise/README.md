# sim2d_noise

FastNoise2 bindings for WASM via Emscripten.

## Overview

This crate wraps [FastNoise2](https://github.com/Auburn/FastNoise2) for use in WASM builds. It compiles to `wasm32-unknown-emscripten` and exports C functions callable from JavaScript.

On native platforms, `bevy_pixel_world` uses FastNoise2 directly. This crate exists solely for WASM support.

## Build Requirements

- Emscripten SDK
- Rust target: `wasm32-unknown-emscripten`

```bash
# Install Emscripten target
rustup target add wasm32-unknown-emscripten

# Activate Emscripten environment
source ~/emsdk/emsdk_env.sh
```

## Building

```bash
# Incremental build (rebuilds only if sources changed)
make

# Force rebuild
make build

# Clean
make clean
```

Output is placed in `dist/`:
- `sim2d_noise.js` - ES6 module loader
- `sim2d_noise.wasm` - WASM binary

## Workspace Exclusion

This crate is excluded from the workspace because it requires a separate build toolchain (Emscripten). The WASM game build uses pre-built artifacts from `dist/`.
