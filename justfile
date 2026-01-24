# Run painting example with dev profile (dynamic linking, visual debug, diagnostics)
dev:
    cargo run -p bevy_pixel_world --example painting

# Run painting example with release profile (diagnostics only for FPS)
run:
    cargo run -p bevy_pixel_world --example painting --release --no-default-features --features diagnostics
