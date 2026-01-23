//! Text Demo - Font rendering verification example.
//!
//! Renders text at various sizes and colors to verify:
//! - Text renders correctly (not upside down or mirrored)
//! - Different font sizes work
//! - Colors apply properly
//! - Position coordinates behave as expected (Y+ up)
//!
//! Run with: `cargo run -p pixel_world --example text_demo`

use bevy::prelude::*;
use pixel_world::{
  draw_text, spawn_static_chunk, Blitter, Chunk, ChunkMaterial, CpuFont, PixelWorldPlugin, Rgba,
};

const CHUNK_SIZE: u32 = 256;

fn main() {
  App::new()
    .add_plugins(DefaultPlugins.set(WindowPlugin {
      primary_window: Some(Window {
        title: "Text Demo - Font Rendering".to_string(),
        resolution: (512, 512).into(),
        ..default()
      }),
      ..default()
    }))
    .add_plugins(PixelWorldPlugin)
    .add_systems(Startup, setup)
    .run();
}

fn setup(
  mut commands: Commands,
  mut images: ResMut<Assets<Image>>,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<ChunkMaterial>>,
) {
  commands.spawn(Camera2d);

  let mut chunk = Chunk::new(CHUNK_SIZE, CHUNK_SIZE);

  // Clear to dark gray
  Blitter::new(chunk.render_surface_mut()).clear(Rgba::rgb(32, 32, 32));

  let font = CpuFont::default_font();

  // Draw text at various sizes and positions
  // Bottom text - small (16px)
  draw_text(
    chunk.render_surface_mut(),
    &font,
    "Small 16px",
    10,
    10,
    16.0,
    0.0,
    Rgba::WHITE,
  );

  // Middle text - medium (24px), different color
  draw_text(
    chunk.render_surface_mut(),
    &font,
    "Medium 24px",
    10,
    60,
    24.0,
    0.0,
    Rgba::rgb(255, 255, 0),
  );

  // Upper text - large (32px)
  draw_text(
    chunk.render_surface_mut(),
    &font,
    "Large 32px",
    10,
    120,
    32.0,
    0.0,
    Rgba::rgb(0, 255, 255),
  );

  // Test Y+ up: "TOP" should appear at top, "BOTTOM" at bottom
  draw_text(
    chunk.render_surface_mut(),
    &font,
    "TOP",
    200,
    220,
    16.0,
    0.0,
    Rgba::rgb(0, 255, 0),
  );

  draw_text(
    chunk.render_surface_mut(),
    &font,
    "BOTTOM",
    200,
    20,
    16.0,
    0.0,
    Rgba::rgb(255, 0, 0),
  );

  // Test character spacing
  draw_text(
    chunk.render_surface_mut(),
    &font,
    "S P A C E D",
    10,
    180,
    16.0,
    4.0,
    Rgba::rgb(255, 128, 0),
  );

  spawn_static_chunk(
    &mut commands,
    &mut images,
    &mut meshes,
    &mut materials,
    chunk.render_surface(),
    Vec2::splat(CHUNK_SIZE as f32 * 2.0),
  );
}
