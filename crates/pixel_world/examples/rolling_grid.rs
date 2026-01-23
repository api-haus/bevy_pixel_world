//! Rolling Grid Demo - Phase 1 streaming window example.
//!
//! Demonstrates the rolling chunk grid with WASD camera movement.
//! Chunks are streamed in/out as the camera moves through the infinite world.
//!
//! Controls:
//! - WASD/Arrow keys: Move camera
//! - Shift: Speed boost (5x)
//!
//! Run with: `cargo run -p pixel_world --example rolling_grid`

use bevy::prelude::*;
use pixel_world::{
  create_chunk_quad, create_texture, draw_text, upload_surface, ChunkMaterial, ChunkPos,
  ChunkSeeder, CpuFont, NoiseSeeder, PixelWorldPlugin, Rgba, StreamingWindow, WorldPos,
  CHUNK_SIZE, WINDOW_HEIGHT, WINDOW_WIDTH,
};

/// Base camera movement speed in pixels per second.
const CAMERA_SPEED: f32 = 500.0;

/// Speed multiplier when holding shift.
const SPEED_BOOST: f32 = 5.0;

fn main() {
  App::new()
    .add_plugins(DefaultPlugins.set(WindowPlugin {
      primary_window: Some(Window {
        title: "Rolling Grid Demo - Phase 1".to_string(),
        resolution: (1280, 720).into(),
        ..default()
      }),
      ..default()
    }))
    .add_plugins(PixelWorldPlugin)
    .add_systems(Startup, setup)
    .add_systems(Update, (camera_input, update_streaming, upload_chunks).chain())
    .run();
}

/// Marker component for the main camera.
#[derive(Component)]
struct MainCamera;

/// Font resource for chunk labels.
#[derive(Resource)]
struct LabelFont(CpuFont);

fn setup(
  mut commands: Commands,
  mut images: ResMut<Assets<Image>>,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<ChunkMaterial>>,
) {
  // Spawn camera at origin
  commands.spawn((Camera2d, MainCamera));

  // Create font for labels
  let font = CpuFont::default_font();
  commands.insert_resource(LabelFont(font));

  // Create noise seeder for terrain generation
  let seeder = NoiseSeeder::new(42, 200.0);

  // Create streaming window
  let mut window = StreamingWindow::new();

  // Create shared mesh for all chunks
  let mesh_handle = meshes.add(create_chunk_quad(CHUNK_SIZE as f32, CHUNK_SIZE as f32));
  commands.insert_resource(ChunkMesh(mesh_handle.clone()));

  // Spawn initial chunks
  let initial_positions: Vec<_> = visible_positions(ChunkPos(0, 0)).collect();
  for pos in initial_positions {
    spawn_chunk(
      &mut commands,
      &mut images,
      &mut materials,
      &mut window,
      pos,
      &mesh_handle,
      &seeder,
    );
  }

  commands.insert_resource(window);
  commands.insert_resource(seeder);
}

/// Shared mesh handle for chunk quads.
#[derive(Resource)]
struct ChunkMesh(Handle<Mesh>);

/// Returns iterator over visible chunk positions for a given center.
fn visible_positions(center: ChunkPos) -> impl Iterator<Item = ChunkPos> {
  let hw = WINDOW_WIDTH as i32 / 2;
  let hh = WINDOW_HEIGHT as i32 / 2;

  let x_range = (center.0 - hw)..(center.0 + hw);
  let y_range = (center.1 - hh)..(center.1 + hh);

  x_range.flat_map(move |x| y_range.clone().map(move |y| ChunkPos(x, y)))
}

/// Spawns a chunk entity at the given position.
fn spawn_chunk(
  commands: &mut Commands,
  images: &mut Assets<Image>,
  materials: &mut Assets<ChunkMaterial>,
  window: &mut StreamingWindow,
  pos: ChunkPos,
  mesh: &Handle<Mesh>,
  seeder: &NoiseSeeder,
) {
  // Acquire chunk from pool
  let handle = match window.acquire_chunk() {
    Some(h) => h,
    None => {
      eprintln!("Pool exhausted at {:?}", pos);
      return;
    }
  };

  // Set position and seed with noise
  let chunk = window.pool.get_mut(handle);
  chunk.set_pos(pos);
  seeder.seed(pos, chunk);

  // Create texture
  let texture = create_texture(images, CHUNK_SIZE, CHUNK_SIZE);

  // Create material
  let material = materials.add(ChunkMaterial {
    texture: Some(texture.clone()),
  });

  // Spawn entity at chunk world position
  let world_pos = pos.to_world();
  let transform = Transform::from_xyz(
    world_pos.0 as f32 + CHUNK_SIZE as f32 / 2.0,
    world_pos.1 as f32 + CHUNK_SIZE as f32 / 2.0,
    0.0,
  );

  let entity = commands
    .spawn((Mesh2d(mesh.clone()), transform, Visibility::default(), MeshMaterial2d(material)))
    .id();

  // Register in window
  window.register_active(pos, handle, entity, texture);
}

fn camera_input(
  keys: Res<ButtonInput<KeyCode>>,
  mut camera: Query<&mut Transform, With<MainCamera>>,
  time: Res<Time>,
) {
  let mut direction = Vec2::ZERO;

  // WASD movement
  if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
    direction.y += 1.0;
  }
  if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
    direction.y -= 1.0;
  }
  if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
    direction.x -= 1.0;
  }
  if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
    direction.x += 1.0;
  }

  if direction == Vec2::ZERO {
    return;
  }

  // Normalize and apply speed
  let direction = direction.normalize();
  let speed = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
    CAMERA_SPEED * SPEED_BOOST
  } else {
    CAMERA_SPEED
  };

  // Update camera position
  if let Ok(mut transform) = camera.single_mut() {
    transform.translation.x += direction.x * speed * time.delta_secs();
    transform.translation.y += direction.y * speed * time.delta_secs();
  }
}

fn update_streaming(
  mut commands: Commands,
  camera: Query<&Transform, With<MainCamera>>,
  mut window: ResMut<StreamingWindow>,
  mut images: ResMut<Assets<Image>>,
  mut materials: ResMut<Assets<ChunkMaterial>>,
  chunk_mesh: Res<ChunkMesh>,
  seeder: Res<NoiseSeeder>,
) {
  let Ok(camera_transform) = camera.single() else {
    return;
  };

  // Convert camera position to chunk position.
  // Offset by half chunk so transitions occur at chunk centers, not edges.
  // This makes the streaming window feel centered on the camera.
  let half_chunk = (CHUNK_SIZE / 2) as i64;
  let cam_x = camera_transform.translation.x as i64 + half_chunk;
  let cam_y = camera_transform.translation.y as i64 + half_chunk;
  let (chunk_pos, _) = WorldPos(cam_x, cam_y).to_chunk_and_local();

  // Update window center
  let delta = window.update_center(chunk_pos);

  // Despawn old chunks
  for (_, entity) in delta.to_despawn {
    commands.entity(entity).despawn();
  }

  // Spawn new chunks
  for pos in delta.to_spawn {
    // Acquire chunk from pool
    let handle = match window.acquire_chunk() {
      Some(h) => h,
      None => {
        eprintln!("Pool exhausted at {:?}", pos);
        continue;
      }
    };

    // Set position and seed with noise
    let chunk = window.pool.get_mut(handle);
    chunk.set_pos(pos);
    seeder.seed(pos, chunk);

    // Create texture
    let texture = create_texture(&mut images, CHUNK_SIZE, CHUNK_SIZE);

    // Create material
    let material = materials.add(ChunkMaterial {
      texture: Some(texture.clone()),
    });

    // Spawn entity at chunk world position
    let world_pos = pos.to_world();
    let transform = Transform::from_xyz(
      world_pos.0 as f32 + CHUNK_SIZE as f32 / 2.0,
      world_pos.1 as f32 + CHUNK_SIZE as f32 / 2.0,
      0.0,
    );

    let entity = commands
      .spawn((
        Mesh2d(chunk_mesh.0.clone()),
        transform,
        Visibility::default(),
        MeshMaterial2d(material),
      ))
      .id();

    // Register in window
    window.register_active(pos, handle, entity, texture);
  }
}

fn upload_chunks(mut window: ResMut<StreamingWindow>, mut images: ResMut<Assets<Image>>, font: Res<LabelFont>) {
  // Collect dirty chunks with needed data
  let dirty_chunks: Vec<_> = window
    .active
    .iter()
    .filter(|(_, active)| active.dirty)
    .map(|(pos, active)| (*pos, active.handle, active.texture.clone()))
    .collect();

  for (pos, handle, texture_handle) in dirty_chunks {
    let chunk = window.pool.get_mut(handle);

    // Draw chunk position label
    let label = format!("({}, {})", pos.0, pos.1);
    draw_text(&mut chunk.pixels, &font.0, &label, 10, 10, 16.0, 0.0, Rgba::WHITE);

    // Upload to GPU
    if let Some(image) = images.get_mut(&texture_handle) {
      upload_surface(&chunk.pixels, image);
    }

    // Mark clean
    if let Some(active) = window.active.get_mut(&pos) {
      active.dirty = false;
    }
  }
}
