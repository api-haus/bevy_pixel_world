//! Gremlins stress test — randomly fires actions each tick to surface crashes.
//!
//! Run: cargo test -p bevy_pixel_world gremlins_stress

use std::path::Path;
use std::time::{Duration, Instant};

use bevy::app::{TaskPoolOptions, TaskPoolPlugin};
use bevy::prelude::*;
use bevy_pixel_world::{
  ColorIndex, DisplacementState, LastBlitTransform, MaterialSeeder, Persistable, PersistenceConfig,
  Pixel, PixelBodiesPlugin, PixelBody, PixelBodyIdGenerator, PixelWorld, PixelWorldPlugin,
  SpawnPixelWorld, StreamingCamera, WorldPos, WorldRect, material_ids,
};
use rand::prelude::*;
use tempfile::TempDir;

struct TestHarness {
  app: App,
  camera: Entity,
}

impl TestHarness {
  fn new(save_path: &Path) -> Self {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins.set(TaskPoolPlugin {
      task_pool_options: TaskPoolOptions::with_num_threads(4),
    }));

    app.add_plugins(bevy::transform::TransformPlugin);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::image::ImagePlugin::default());
    app.add_plugins(bevy::scene::ScenePlugin);
    app.add_plugins(bevy::gizmos::GizmoPlugin);

    app.add_plugins(
      PixelWorldPlugin::default().persistence(PersistenceConfig::new("test").with_path(save_path)),
    );
    app.add_plugins(PixelBodiesPlugin);

    let camera = app
      .world_mut()
      .spawn((
        Transform::default(),
        GlobalTransform::default(),
        StreamingCamera,
      ))
      .id();

    app
      .world_mut()
      .commands()
      .queue(SpawnPixelWorld::new(MaterialSeeder::new(42)));

    app.update();

    Self { app, camera }
  }

  fn run_until_seeded(&mut self) {
    for i in 0..100 {
      self.app.update();
      if i % 20 == 19 {
        let mut q = self.app.world_mut().query::<&PixelWorld>();
        if let Ok(world) = q.single(self.app.world()) {
          if world.get_pixel(WorldPos::new(0, 0)).is_some() {
            return;
          }
        }
      }
    }
  }

  fn move_camera(&mut self, position: Vec3) {
    let mut transform = self
      .app
      .world_mut()
      .get_mut::<Transform>(self.camera)
      .unwrap();
    transform.translation = position;
    drop(transform);
    let mut global = self
      .app
      .world_mut()
      .get_mut::<GlobalTransform>(self.camera)
      .unwrap();
    *global = GlobalTransform::from(Transform::from_translation(position));
  }

  fn camera_position(&self) -> Vec3 {
    self
      .app
      .world()
      .get::<Transform>(self.camera)
      .unwrap()
      .translation
  }
}

struct GremlinState {
  rng: StdRng,
  idle_until: Option<Instant>,
  tick: u64,
  total_distance: f32,
  // Spiraloid path parameters
  time: f32,
  phase_offset: f32,
}

impl GremlinState {
  fn new(seed: u64) -> Self {
    let mut rng = StdRng::seed_from_u64(seed);
    let phase_offset = rng.gen_range(0.0..std::f32::consts::TAU);
    Self {
      rng,
      idle_until: None,
      tick: 0,
      total_distance: 0.0,
      time: 0.0,
      phase_offset,
    }
  }

  fn is_idle(&self) -> bool {
    self.idle_until.is_some_and(|t| Instant::now() < t)
  }

  fn maybe_start_idle(&mut self) {
    // 5% chance to enter idle period (reduced from 10%)
    if self.rng.gen_ratio(1, 20) {
      let duration_ms = self.rng.gen_range(200..=800);
      self.idle_until = Some(Instant::now() + Duration::from_millis(duration_ms));
    }
  }

  /// Rose curve / rhodonea path that returns to origin periodically
  /// r = max_radius * |sin(k * theta)|, with theta advancing over time
  /// k=3 gives a 3-petal rose that passes through origin 6 times per cycle
  fn compute_target_position(&self) -> Vec2 {
    let theta = self.time + self.phase_offset;
    let k = 3.0; // 3-petal rose
    let max_radius = 2000.0; // Max distance from origin

    // Rose curve: r = max_radius * |sin(k * theta)|
    // This creates petals that sweep out from and return to origin
    let r = max_radius * (k * theta).sin().abs();

    Vec2::new(r * theta.cos(), r * theta.sin())
  }

  /// Advance time parameter for smooth, fast movement
  fn advance_time(&mut self) {
    // Speed: complete roughly 2 full rose cycles in 15 seconds at ~300 ticks
    // Each cycle = 2*PI, so 4*PI total / 300 ticks ≈ 0.042 per tick
    // Bump it up for more aggressive movement
    self.time += 0.08;
  }
}

const MATERIALS: [bevy_pixel_world::MaterialId; 4] = [
  material_ids::SOIL,
  material_ids::STONE,
  material_ids::SAND,
  material_ids::WOOD,
];

fn gremlin_spawn_body(harness: &mut TestHarness, rng: &mut StdRng) {
  let x = rng.gen_range(-200.0..200.0f32);
  let y = rng.gen_range(-200.0..200.0f32);
  let size = rng.gen_range(4..=16u32);
  let material = MATERIALS[rng.gen_range(0..MATERIALS.len())];

  let mut body = PixelBody::new(size, size);
  for py in 0..size {
    for px in 0..size {
      body.set_pixel(px, py, Pixel::new(material, ColorIndex(100)));
    }
  }

  let body_id = {
    let mut id_gen = harness
      .app
      .world_mut()
      .resource_mut::<PixelBodyIdGenerator>();
    id_gen.generate()
  };

  let transform = Transform::from_translation(Vec2::new(x, y).extend(0.0));
  let global_transform = GlobalTransform::from(transform);

  harness.app.world_mut().spawn((
    body,
    LastBlitTransform::default(),
    DisplacementState::default(),
    transform,
    global_transform,
    body_id,
    Persistable,
  ));
}

fn gremlin_destroy_body(harness: &mut TestHarness, rng: &mut StdRng) {
  let bodies: Vec<Entity> = harness
    .app
    .world_mut()
    .query_filtered::<Entity, With<PixelBody>>()
    .iter(harness.app.world())
    .collect();

  if !bodies.is_empty() {
    let idx = rng.gen_range(0..bodies.len());
    let entity = bodies[idx];
    harness.app.world_mut().despawn(entity);
  }
}

fn gremlin_pan_camera(harness: &mut TestHarness, rng: &mut StdRng) {
  let current = harness.camera_position();
  let dx = rng.gen_range(-20.0..20.0f32);
  let dy = rng.gen_range(-20.0..20.0f32);
  harness.move_camera(current + Vec3::new(dx, dy, 0.0));
}

fn gremlin_paint_material(harness: &mut TestHarness, rng: &mut StdRng) {
  let cx = rng.gen_range(-200..200i64);
  let cy = rng.gen_range(-200..200i64);
  let radius = rng.gen_range(5..=20i64);
  let material = MATERIALS[rng.gen_range(0..MATERIALS.len())];
  let color_idx = ColorIndex(rng.gen_range(50..200));
  let pixel = Pixel::new(material, color_idx);
  let rect = WorldRect::centered(cx, cy, radius as u32);

  let mut q = harness.app.world_mut().query::<&mut PixelWorld>();
  if let Ok(mut world) = q.single_mut(harness.app.world_mut()) {
    world.blit(
      rect,
      |frag| {
        let dx = frag.x - cx;
        let dy = frag.y - cy;
        if dx * dx + dy * dy <= radius * radius {
          Some(pixel)
        } else {
          None
        }
      },
      Default::default(),
    );
  }
}

fn gremlin_paint_void(harness: &mut TestHarness, rng: &mut StdRng) {
  let cx = rng.gen_range(-200..200i64);
  let cy = rng.gen_range(-200..200i64);
  let radius = rng.gen_range(5..=20i64);
  let rect = WorldRect::centered(cx, cy, radius as u32);
  let void = Pixel::VOID;

  let mut q = harness.app.world_mut().query::<&mut PixelWorld>();
  if let Ok(mut world) = q.single_mut(harness.app.world_mut()) {
    world.blit(
      rect,
      |frag| {
        let dx = frag.x - cx;
        let dy = frag.y - cy;
        if dx * dx + dy * dy <= radius * radius {
          Some(void)
        } else {
          None
        }
      },
      Default::default(),
    );
  }
}

fn gremlin_paint_heat(harness: &mut TestHarness, rng: &mut StdRng) {
  let cx = rng.gen_range(-200..200i64);
  let cy = rng.gen_range(-200..200i64);
  let radius = rng.gen_range(5..=20i64);
  let heat = rng.gen_range(50..=255u8);

  let mut q = harness.app.world_mut().query::<&mut PixelWorld>();
  if let Ok(mut world) = q.single_mut(harness.app.world_mut()) {
    for dy in -radius..=radius {
      for dx in -radius..=radius {
        if dx * dx + dy * dy <= radius * radius {
          world.set_heat_at(WorldPos::new(cx + dx, cy + dy), heat);
        }
      }
    }
  }
}

const SEEDS: [u64; 5] = [
  0xDEAD_BEEF,
  0xCAFE_BABE,
  0xFEED_FACE,
  0xBADC_0FFE,
  0x1337_C0DE,
];

fn run_gremlins_with_seed(seed: u64, duration_secs: u64) {
  let temp_dir = TempDir::new().unwrap();
  let save_path = temp_dir.path().join("gremlins.save");

  let mut harness = TestHarness::new(&save_path);
  harness.move_camera(Vec3::ZERO);
  harness.run_until_seeded();

  let mut state = GremlinState::new(seed);
  let deadline = Instant::now() + Duration::from_secs(duration_secs);

  let mut last_pos = harness.camera_position().truncate();

  while Instant::now() < deadline {
    // Compute target position on spiraloid path
    let target_pos = state.compute_target_position();
    harness.move_camera(target_pos.extend(0.0));

    // Track distance
    let distance_this_tick = (target_pos - last_pos).length();
    state.total_distance += distance_this_tick;
    last_pos = target_pos;

    // Advance along the path
    state.advance_time();

    // Check if in idle period
    if state.is_idle() {
      harness.app.update();
      state.tick += 1;
      continue;
    }

    // Maybe start idle period
    state.maybe_start_idle();

    // Run random actions
    let action_count = state.rng.gen_range(1..=3usize);
    for _ in 0..action_count {
      match state.rng.gen_range(0..6u32) {
        0 => gremlin_spawn_body(&mut harness, &mut state.rng),
        1 => gremlin_destroy_body(&mut harness, &mut state.rng),
        2 => gremlin_pan_camera(&mut harness, &mut state.rng),
        3 => gremlin_paint_material(&mut harness, &mut state.rng),
        4 => gremlin_paint_void(&mut harness, &mut state.rng),
        5 => gremlin_paint_heat(&mut harness, &mut state.rng),
        _ => unreachable!(),
      }
    }
    harness.app.update();
    state.tick += 1;
  }
  let final_pos = harness.camera_position();
  eprintln!(
    "gremlins: seed {:#X} | {} ticks | distance: {:.0} | final pos: ({:.0}, {:.0})",
    seed, state.tick, state.total_distance, final_pos.x, final_pos.y
  );
}

#[test]
fn gremlins() {
  // Run multiple iterations with different seeds to increase coverage
  for (i, &seed) in SEEDS.iter().enumerate() {
    eprintln!(
      "gremlins: starting run {}/{} with seed {:#X}",
      i + 1,
      SEEDS.len(),
      seed
    );
    run_gremlins_with_seed(seed, 15);
  }
  // If we got here without panicking, the test passes.
}
