# Run painting example with dev profile (dynamic linking, visual debug, diagnostics)
dev:
    cargo run -p bevy_pixel_world --example painting --features avian2d

dev_rapier:
    cargo run -p bevy_pixel_world --example painting --features rapier2d

# Run painting example with release profile (diagnostics only for FPS)
run:
    cargo run -p bevy_pixel_world --example painting --release --no-default-features --features diagnostics,visual-debug,avian2d

run_rapier2d:
    cargo run -p bevy_pixel_world --example painting --release --no-default-features --features diagnostics,visual-debug,rapier2d

game:
    cargo run -p game
