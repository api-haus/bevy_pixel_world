run: game

test:
    cargo test -p bevy_pixel_world

dev:
    cargo run -p game --features dev

game:
    cargo run -p game --release

# Serve game with trunk (WASM dev server)
serve:
    cd crates/game && trunk serve
