run: game

test:
    cargo test -p bevy_pixel_world

dev:
    cd crates/game && cargo run --features dev

game:
    cd crates/game && cargo run --release

# Serve game with trunk (WASM dev server)
serve:
    cd crates/game && trunk serve

# Build NoiseTool (NodeEditor) from vendored FastNoise2
build-noise-tool:
    cmake -S vendor/FastNoise2 -B vendor/FastNoise2/build \
        -DFASTNOISE2_TOOLS=ON \
        -DCMAKE_BUILD_TYPE=Release
    cmake --build vendor/FastNoise2/build --target NodeEditor -j

# Run NoiseTool (builds if needed)
noise-tool: build-noise-tool
    ./vendor/FastNoise2/build/Release/bin/NodeEditor
