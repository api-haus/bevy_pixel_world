//! Brush Painting Demo - Parallel tiled blitter test.
//!
//! Demonstrates the Canvas blit API with parallel tile processing.
//! Paint filled circles across chunk boundaries to verify seamless operation.
//!
//! Controls:
//! - LMB: Paint with soil material
//! - RMB: Erase (paint with air)
//! - Scroll wheel: Adjust brush radius
//! - WASD/Arrow keys: Move camera
//! - Shift: Speed boost (5x)
//!
//! Run with: `cargo run -p pixel_world --example painting`

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use pixel_world::{
  create_chunk_quad, create_texture, material_ids, upload_surface, ChunkMaterial, ChunkPos, ChunkSeeder, ColorIndex,
  MaterialSeeder, Materials, Pixel, PixelWorldPlugin, StreamingWindow, WorldPos,
  CHUNK_SIZE, WINDOW_HEIGHT, WINDOW_WIDTH,
};

const CAMERA_SPEED: f32 = 500.0;
const SPEED_BOOST: f32 = 5.0;
const MIN_RADIUS: u32 = 5;
const MAX_RADIUS: u32 = 200;
const DEFAULT_RADIUS: u32 = 20;

fn main() {
  App::new()
    .add_plugins(DefaultPlugins.set(WindowPlugin {
      primary_window: Some(Window {
        title: "Brush Painting Demo - Parallel Blitter".to_string(),
        resolution: (1280, 720).into(),
        ..default()
      }),
      ..default()
    }))
    .add_plugins(PixelWorldPlugin)
    .insert_resource(BrushState::default())
    .add_systems(Startup, setup)
    .add_systems(
      Update,
      (
        input_system,
        camera_input,
        update_streaming,
        paint_system,
        upload_chunks,
      )
        .chain(),
    )
    .run();
}

#[derive(Resource)]
struct BrushState {
  radius: u32,
  painting: bool,
  erasing: bool,
  world_pos: Option<(i64, i64)>,
}

impl Default for BrushState {
  fn default() -> Self {
    Self {
      radius: DEFAULT_RADIUS,
      painting: false,
      erasing: false,
      world_pos: None,
    }
  }
}

#[derive(Component)]
struct MainCamera;

#[derive(Resource)]
struct ChunkMesh(Handle<Mesh>);

fn setup(
  mut commands: Commands,
  mut images: ResMut<Assets<Image>>,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<ChunkMaterial>>,
) {
  commands.spawn((Camera2d, MainCamera));

  let mat_registry = Materials::new();
  commands.insert_resource(mat_registry);

  // Create material seeder for terrain generation (same as rolling_grid)
  // seed=42, feature_scale=200.0, threshold=0.0, soil_depth=8, feather_scale=3.0
  let seeder = MaterialSeeder::new(42, 200.0, 0.0, 8, 3.0);

  let mut window = StreamingWindow::new();
  let mesh_handle = meshes.add(create_chunk_quad(CHUNK_SIZE as f32, CHUNK_SIZE as f32));
  commands.insert_resource(ChunkMesh(mesh_handle.clone()));

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

fn visible_positions(center: ChunkPos) -> impl Iterator<Item = ChunkPos> {
  let hw = WINDOW_WIDTH as i32 / 2;
  let hh = WINDOW_HEIGHT as i32 / 2;
  let x_range = (center.0 - hw)..(center.0 + hw);
  let y_range = (center.1 - hh)..(center.1 + hh);
  x_range.flat_map(move |x| y_range.clone().map(move |y| ChunkPos(x, y)))
}

fn spawn_chunk(
  commands: &mut Commands,
  images: &mut Assets<Image>,
  materials: &mut Assets<ChunkMaterial>,
  window: &mut StreamingWindow,
  pos: ChunkPos,
  mesh: &Handle<Mesh>,
  seeder: &MaterialSeeder,
) {
  let handle = match window.acquire_chunk() {
    Some(h) => h,
    None => {
      eprintln!("Pool exhausted at {:?}", pos);
      return;
    }
  };

  let chunk = window.pool.get_mut(handle);
  chunk.set_pos(pos);
  seeder.seed(pos, chunk);

  let texture = create_texture(images, CHUNK_SIZE, CHUNK_SIZE);
  let material = materials.add(ChunkMaterial {
    texture: Some(texture.clone()),
  });

  let world_pos = pos.to_world();
  let transform = Transform::from_xyz(
    world_pos.0 as f32 + CHUNK_SIZE as f32 / 2.0,
    world_pos.1 as f32 + CHUNK_SIZE as f32 / 2.0,
    0.0,
  );

  let entity = commands
    .spawn((
      Mesh2d(mesh.clone()),
      transform,
      Visibility::default(),
      MeshMaterial2d(material.clone()),
    ))
    .id();

  window.register_active(pos, handle, entity, texture, material);
}

fn input_system(
  mut brush: ResMut<BrushState>,
  mouse_buttons: Res<ButtonInput<MouseButton>>,
  mut scroll_events: MessageReader<MouseWheel>,
  window_query: Query<&Window, With<PrimaryWindow>>,
  camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
) {
  brush.painting = mouse_buttons.pressed(MouseButton::Left);
  brush.erasing = mouse_buttons.pressed(MouseButton::Right);

  // Handle scroll wheel for radius
  for event in scroll_events.read() {
    let delta = match event.unit {
      MouseScrollUnit::Line => event.y as i32 * 5,
      MouseScrollUnit::Pixel => (event.y / 10.0) as i32,
    };
    let new_radius = (brush.radius as i32 + delta).clamp(MIN_RADIUS as i32, MAX_RADIUS as i32);
    brush.radius = new_radius as u32;
  }

  // Convert mouse position to world coordinates
  let Ok(window) = window_query.single() else {
    return;
  };
  let Ok((camera, camera_transform)) = camera_query.single() else {
    return;
  };

  if let Some(cursor_pos) = window.cursor_position() {
    if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
      brush.world_pos = Some((world_pos.x as i64, world_pos.y as i64));
    }
  } else {
    brush.world_pos = None;
  }
}

fn camera_input(
  keys: Res<ButtonInput<KeyCode>>,
  mut camera: Query<&mut Transform, With<MainCamera>>,
  time: Res<Time>,
) {
  let mut direction = Vec2::ZERO;

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

  let direction = direction.normalize();
  let speed = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
    CAMERA_SPEED * SPEED_BOOST
  } else {
    CAMERA_SPEED
  };

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
  seeder: Res<MaterialSeeder>,
) {
  let Ok(camera_transform) = camera.single() else {
    return;
  };

  let half_chunk = (CHUNK_SIZE / 2) as i64;
  let cam_x = camera_transform.translation.x as i64 + half_chunk;
  let cam_y = camera_transform.translation.y as i64 + half_chunk;
  let (chunk_pos, _) = WorldPos(cam_x, cam_y).to_chunk_and_local();

  let delta = window.update_center(chunk_pos);

  for (_, entity) in delta.to_despawn {
    commands.entity(entity).despawn();
  }

  for pos in delta.to_spawn {
    let handle = match window.acquire_chunk() {
      Some(h) => h,
      None => {
        eprintln!("Pool exhausted at {:?}", pos);
        continue;
      }
    };

    let chunk = window.pool.get_mut(handle);
    chunk.set_pos(pos);
    seeder.seed(pos, chunk);

    let texture = create_texture(&mut images, CHUNK_SIZE, CHUNK_SIZE);
    let material = materials.add(ChunkMaterial {
      texture: Some(texture.clone()),
    });

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
        MeshMaterial2d(material.clone()),
      ))
      .id();

    window.register_active(pos, handle, entity, texture, material);
  }
}

fn paint_system(brush: Res<BrushState>, mut window: ResMut<StreamingWindow>) {
  if !brush.painting && !brush.erasing {
    return;
  }

  let Some((center_x, center_y)) = brush.world_pos else {
    return;
  };

  // Use STONE material for painting, AIR for erasing
  let (material, color) = if brush.painting {
    (material_ids::STONE, ColorIndex(128))
  } else {
    (material_ids::AIR, ColorIndex(0))
  };
  let pixel = Pixel::new(material, color);

  let radius = brush.radius as i64;
  let radius_sq = radius * radius;

  // Circular brush
  for dy in -radius..=radius {
    for dx in -radius..=radius {
      // Circle distance check
      if dx * dx + dy * dy > radius_sq {
        continue;
      }

      let world_x = center_x + dx;
      let world_y = center_y + dy;

      let (chunk_pos, local_pos) = WorldPos(world_x, world_y).to_chunk_and_local();

      let handle = window.active.get(&chunk_pos).map(|a| a.handle);
      if let Some(handle) = handle {
        let chunk = window.pool.get_mut(handle);
        chunk.pixels[(local_pos.0 as u32, local_pos.1 as u32)] = pixel;
      }
    }
  }

  // Mark affected chunks dirty
  let min_chunk_x = ((center_x - radius) as f64 / CHUNK_SIZE as f64).floor() as i32;
  let max_chunk_x = ((center_x + radius) as f64 / CHUNK_SIZE as f64).floor() as i32;
  let min_chunk_y = ((center_y - radius) as f64 / CHUNK_SIZE as f64).floor() as i32;
  let max_chunk_y = ((center_y + radius) as f64 / CHUNK_SIZE as f64).floor() as i32;

  for cy in min_chunk_y..=max_chunk_y {
    for cx in min_chunk_x..=max_chunk_x {
      window.mark_dirty(ChunkPos(cx, cy));
    }
  }
}

fn upload_chunks(
  mut window: ResMut<StreamingWindow>,
  mut images: ResMut<Assets<Image>>,
  mut materials: ResMut<Assets<ChunkMaterial>>,
  mat_registry: Res<Materials>,
) {
  let dirty_chunks: Vec<_> = window
    .active
    .iter()
    .filter(|(_, active)| active.dirty)
    .map(|(pos, active)| {
      (
        *pos,
        active.handle,
        active.texture.clone(),
        active.material.clone(),
      )
    })
    .collect();

  for (pos, handle, texture_handle, material_handle) in dirty_chunks {
    let chunk = window.pool.get_mut(handle);
    chunk.materialize(&mat_registry);

    if let Some(image) = images.get_mut(&texture_handle) {
      upload_surface(chunk.render_surface(), image);
    }

    // Touch the material to force bind group refresh (workaround for Bevy bug
    // #15081)
    let _ = materials.get_mut(&material_handle);

    if let Some(active) = window.active.get_mut(&pos) {
      active.dirty = false;
    }
  }
}
