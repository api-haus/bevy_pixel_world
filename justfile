# Run painting example with dev profile (dynamic linking, visual debug, diagnostics)
dev_avian2d:
    cargo run -p bevy_pixel_world --example painting --features avian2d,dev

dev_rapier2d:
    cargo run -p bevy_pixel_world --example painting --features rapier2d,dev

dev: dev_rapier2d

# Run painting example with release profile (diagnostics only for FPS)
run_avian2d:
    cargo run -p bevy_pixel_world --example painting --release --features avian2d

run_rapier2d:
    cargo run -p bevy_pixel_world --example painting --release --features rapier2d

# Run all bevy_pixel_world E2E and unit tests (headless, no GPU)
test-pixel-world:
    cargo test -p bevy_pixel_world --features headless --no-default-features

game:
    cargo run -p game
