//! UV Quad Demo - Phase 0 verification example.
//!
//! Renders an animated UV-colored quad at 60 TPS.
//! - Red increases to the right (U coordinate)
//! - Green increases upward (V coordinate)
//! - Blue channel pulses over time
//!
//! Run with: `cargo run -p pixel_world --example uv_quad`

use bevy::prelude::*;
use pixel_world::core::rect::Rect;
use pixel_world::{
  create_chunk_quad, create_texture, upload_surface, Blitter, Chunk, ChunkMaterial, PixelWorldPlugin,
  Rgba,
};

/// Size of the chunk in pixels.
const CHUNK_SIZE: u32 = 256;

/// Size of the bouncing quad.
const QUAD_SIZE: u32 = 64;

/// Pixels per second for quad movement.
const MOVE_SPEED: f32 = 100.0;

fn main() {
  App::new()
    .add_plugins(DefaultPlugins.set(WindowPlugin {
      primary_window: Some(Window {
        title: "UV Quad Demo - Phase 0".to_string(),
        resolution: (512, 512).into(),
        ..default()
      }),
      ..default()
    }))
    .add_plugins(PixelWorldPlugin)
    .insert_resource(Time::<Fixed>::from_hz(60.0))
    .add_systems(Startup, setup)
    .add_systems(FixedUpdate, update_quad)
    .add_systems(Update, upload_to_gpu)
    .run();
}

/// Holds the chunk and animation state.
#[derive(Resource)]
struct UvQuadState {
  chunk: Chunk,
  texture_handle: Handle<Image>,
  material_handle: Handle<ChunkMaterial>,
  /// Position of the quad (bottom-left corner).
  pos: Vec2,
  /// Velocity of the quad.
  vel: Vec2,
  /// Time accumulator for blue pulse.
  time: f32,
  /// Whether the chunk needs uploading.
  dirty: bool,
}

fn setup(
  mut commands: Commands,
  mut images: ResMut<Assets<Image>>,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<ChunkMaterial>>,
) {
  // Create camera
  commands.spawn(Camera2d);

  // Create chunk and texture
  let chunk = Chunk::new(CHUNK_SIZE, CHUNK_SIZE);
  let texture_handle = create_texture(&mut images, CHUNK_SIZE, CHUNK_SIZE);

  // Create quad mesh with Y+ up UVs
  let mesh_handle = meshes.add(create_chunk_quad(
    CHUNK_SIZE as f32 * 2.0,
    CHUNK_SIZE as f32 * 2.0,
  ));
  let material_handle = materials.add(ChunkMaterial {
    texture: Some(texture_handle.clone()),
  });

  // Spawn mesh with chunk material
  commands.spawn((Mesh2d(mesh_handle), MeshMaterial2d(material_handle.clone())));

  // Initialize state
  commands.insert_resource(UvQuadState {
    chunk,
    texture_handle,
    material_handle,
    pos: Vec2::new(
      (CHUNK_SIZE - QUAD_SIZE) as f32 / 2.0,
      (CHUNK_SIZE - QUAD_SIZE) as f32 / 2.0,
    ),
    vel: Vec2::new(MOVE_SPEED, MOVE_SPEED * 0.7),
    time: 0.0,
    dirty: true,
  });
}

fn update_quad(mut state: ResMut<UvQuadState>, time: Res<Time<Fixed>>) {
  let dt = time.delta_secs();
  state.time += dt;

  // Update position - read vel before mutating pos
  let vel = state.vel;
  state.pos += vel * dt;

  // Bounce off edges
  let max_x = (CHUNK_SIZE - QUAD_SIZE) as f32;
  let max_y = (CHUNK_SIZE - QUAD_SIZE) as f32;

  if state.pos.x <= 0.0 {
    state.pos.x = 0.0;
    state.vel.x = state.vel.x.abs();
  } else if state.pos.x >= max_x {
    state.pos.x = max_x;
    state.vel.x = -state.vel.x.abs();
  }

  if state.pos.y <= 0.0 {
    state.pos.y = 0.0;
    state.vel.y = state.vel.y.abs();
  } else if state.pos.y >= max_y {
    state.pos.y = max_y;
    state.vel.y = -state.vel.y.abs();
  }

  // Read values before creating blitter
  let pos_x = state.pos.x as u32;
  let pos_y = state.pos.y as u32;
  let blue_pulse = ((state.time * 2.0).sin() * 0.5 + 0.5) * 255.0;
  let blue = blue_pulse as u8;

  // Clear to black
  let mut blitter = Blitter::new(&mut state.chunk.pixels);
  blitter.clear(Rgba::BLACK);

  // Blit UV quad
  let rect = Rect::new(pos_x, pos_y, QUAD_SIZE, QUAD_SIZE);
  blitter.blit(rect, |_x, _y, u, v| {
    Rgba::new(
      (u * 255.0) as u8, // Red increases right
      (v * 255.0) as u8, // Green increases up
      blue,              // Blue pulses
      255,
    )
  });

  state.dirty = true;
}

fn upload_to_gpu(
  mut state: ResMut<UvQuadState>,
  mut images: ResMut<Assets<Image>>,
  mut materials: ResMut<Assets<ChunkMaterial>>,
) {
  if !state.dirty {
    return;
  }

  if let Some(image) = images.get_mut(&state.texture_handle) {
    upload_surface(&state.chunk.pixels, image);
    // Touch the material to force bind group refresh (workaround for Bevy bug
    // #15081)
    let _ = materials.get_mut(&state.material_handle);
    state.dirty = false;
  }
}
