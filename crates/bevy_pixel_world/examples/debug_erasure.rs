//! Debug Erasure - Visual test for pixel body erasure and chunk repositioning.
//!
//! This example runs three automated test cases:
//! 1. Stability test: Spawn bodies, verify they don't disintegrate
//! 2. Erasure test: Spawn bodies, erase them with brush, verify complete
//!    removal
//! 3. Repositioning test: Spawn bodies, scroll camera away 5 chunks, scroll
//!    back, verify no pixel duplication
//!
//! Controls:
//! - Space: Spawn a body at random position
//! - E: Toggle manual auto-erase cycle
//! - 1: Run stability test (spawn only)
//! - 2: Run erasure test (spawn + erase + verify)
//! - 3: Run repositioning test (spawn + scroll + verify)
//! - WASD: Move camera
//!
//! Run with: `cargo run -p bevy_pixel_world --example debug_erasure --features
//! avian2d`
//!
//! Command line arguments:
//!   --test 1    Run stability test automatically
//!   --test 2    Run erasure test automatically
//!   --test 3    Run repositioning test automatically
//!   --exit      Exit after test completes (for CI)

use bevy::camera::ScalingMode;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
#[cfg(feature = "avian2d")]
use bevy_pixel_world::SpawnPixelBody;
#[cfg(feature = "diagnostics")]
use bevy_pixel_world::diagnostics::DiagnosticsPlugin;
use bevy_pixel_world::pixel_body::{LastBlitTransform, PixelBody};
use bevy_pixel_world::{
  ColorIndex, MaterialSeeder, PersistenceConfig, Pixel, PixelFlags, PixelWorld, PixelWorldPlugin,
  SpawnPixelWorld, StreamingCamera, WorldPos, WorldRect, material_ids,
};
use rand::Rng;

const CAMERA_SPEED: f32 = 300.0;
const SPAWN_AREA: (f32, f32, f32, f32) = (60.0, 80.0, 140.0, 160.0); // x_min, y_min, x_max, y_max
const BRUSH_RADIUS: i64 = 30; // Brush size for erasure
const PLATFORM_Y: i64 = 20; // Y position of the stone platform
const PLATFORM_WIDTH: i64 = 400; // Width of platform (wide)
const PLATFORM_HEIGHT: i64 = 40; // Height/thickness of platform (4x normal)
const CLEAR_MARGIN: i64 = 100; // Extra margin around visible area to clear

const SPAWN_COUNT: usize = 15; // Number of bodies to spawn
const SETTLE_FRAMES: usize = 120; // Frames to wait for bodies to settle (2 seconds at 60fps)
const VERIFY_FRAMES: usize = 60; // Frames to verify erasure (1 second)

// Repositioning test constants
const CHUNK_SIZE_PX: f32 = 64.0; // Chunk size in pixels
const REPOSITION_DISTANCE: f32 = CHUNK_SIZE_PX * 5.0; // 5 chunks = 320 pixels
const SCROLL_SPEED: f32 = 200.0; // Pixels per second for scrolling
const WAIT_UP_FRAMES: usize = 30; // Frames to wait after scrolling up

/// Command line configuration
#[derive(Resource, Default)]
struct CliConfig {
  /// Test to auto-start (1 = stability, 2 = erasure)
  auto_test: Option<u8>,
  /// Exit after test completes
  exit_on_complete: bool,
}

fn parse_args() -> CliConfig {
  let args: Vec<String> = std::env::args().collect();
  let mut config = CliConfig::default();

  let mut i = 1;
  while i < args.len() {
    match args[i].as_str() {
      "--test" => {
        if i + 1 < args.len() {
          if let Ok(n) = args[i + 1].parse::<u8>() {
            config.auto_test = Some(n);
          }
          i += 1;
        }
      }
      "--exit" => {
        config.exit_on_complete = true;
      }
      _ => {}
    }
    i += 1;
  }

  config
}

fn main() {
  let cli_config = parse_args();

  // Create a fresh temp directory for this run
  let temp_dir = std::env::temp_dir().join(format!("debug_erasure_{}", std::process::id()));
  std::fs::create_dir_all(&temp_dir).expect("Failed to create temp directory");
  let save_path = temp_dir.join("test.save");
  println!("Using temp save file: {}", save_path.display());

  let mut app = App::new();

  app
    .add_plugins(DefaultPlugins.set(WindowPlugin {
      primary_window: Some(Window {
        title: "Debug Erasure - Pixel Body Test".to_string(),
        resolution: (1280, 720).into(),
        ..default()
      }),
      ..default()
    }))
    .add_plugins(
      PixelWorldPlugin::default()
        .persistence(PersistenceConfig::new("debug_erasure").with_path(&save_path)),
    )
    .add_plugins(EguiPlugin::default())
    .insert_resource(cli_config)
    .init_resource::<DebugState>()
    .add_systems(Startup, setup)
    .add_systems(EguiPrimaryContextPass, diagnostic_ui)
    .add_systems(
      Update,
      (
        draw_platform,
        auto_start_test,
        camera_input,
        manual_input,
        run_test_phases,
        check_exit_condition,
      ),
    );

  #[cfg(feature = "diagnostics")]
  app.add_plugins(DiagnosticsPlugin);

  #[cfg(feature = "avian2d")]
  {
    app.add_plugins(avian2d::prelude::PhysicsPlugins::default());
    app.insert_resource(avian2d::prelude::Gravity(Vec2::new(0.0, -500.0)));
  }

  app.run();
}

/// Current phase of the automated test.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TestPhase {
  /// Waiting for user input or platform setup
  Idle,
  /// Spawning bodies
  Spawning,
  /// Waiting for bodies to settle
  Settling,
  /// Erasing with brush sweeps
  Erasing,
  /// Scrolling camera up (repositioning test)
  ScrollingUp,
  /// Waiting after scrolling up (repositioning test)
  WaitingUp,
  /// Scrolling camera back down (repositioning test)
  ScrollingDown,
  /// Verifying complete removal
  Verifying,
  /// Test completed
  Done,
}

/// Type of test being run
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TestType {
  /// Just spawn and observe (stability test)
  StabilityOnly,
  /// Spawn, erase, and verify removal
  FullErasure,
  /// Spawn, scroll away, scroll back, verify no duplicates
  Repositioning,
}

#[derive(Resource)]
struct DebugState {
  /// Current test phase
  phase: TestPhase,
  /// Type of test being run
  test_type: Option<TestType>,
  /// Bodies spawned this session
  bodies_spawned: usize,
  /// Spawn timer
  spawn_timer: Timer,
  /// Frame counter for current phase
  frame_counter: usize,
  /// Current brush position for erasure sweep
  brush_x: i64,
  brush_y: i64,
  /// Log of recent events
  log: Vec<String>,
  /// Max log entries
  max_log: usize,
  /// Platform drawn
  platform_ready: bool,
  /// Verification results
  verify_body_counts: Vec<usize>,
  verify_pixel_counts: Vec<usize>,
  /// Test result
  test_passed: Option<bool>,
  /// Manual auto-erase enabled
  manual_erase: bool,
  manual_erase_index: usize,
  manual_erase_timer: Timer,
  /// Repositioning test: original camera Y position
  original_camera_y: f32,
  /// Repositioning test: body count before scrolling
  pre_scroll_body_count: usize,
  /// Repositioning test: pixel count before scrolling
  pre_scroll_pixel_count: usize,
}

impl Default for DebugState {
  fn default() -> Self {
    Self {
      phase: TestPhase::Idle,
      test_type: None,
      bodies_spawned: 0,
      spawn_timer: Timer::from_seconds(0.15, TimerMode::Repeating),
      frame_counter: 0,
      brush_x: 0,
      brush_y: 0,
      log: Vec::new(),
      max_log: 100,
      platform_ready: false,
      verify_body_counts: Vec::new(),
      verify_pixel_counts: Vec::new(),
      test_passed: None,
      manual_erase: false,
      manual_erase_index: 0,
      manual_erase_timer: Timer::from_seconds(0.05, TimerMode::Repeating),
      original_camera_y: 0.0,
      pre_scroll_body_count: 0,
      pre_scroll_pixel_count: 0,
    }
  }
}

impl DebugState {
  fn log(&mut self, msg: String) {
    info!("{}", msg);
    self.log.push(msg);
    if self.log.len() > self.max_log {
      self.log.remove(0);
    }
  }

  fn start_test(&mut self, test_type: TestType) {
    self.phase = TestPhase::Spawning;
    self.test_type = Some(test_type);
    self.bodies_spawned = 0;
    self.frame_counter = 0;
    self.brush_x = SPAWN_AREA.0 as i64 - CLEAR_MARGIN;
    self.brush_y = PLATFORM_Y + PLATFORM_HEIGHT + BRUSH_RADIUS; // Start above platform
    self.verify_body_counts.clear();
    self.verify_pixel_counts.clear();
    self.test_passed = None;
    self.original_camera_y = 0.0;
    self.pre_scroll_body_count = 0;
    self.pre_scroll_pixel_count = 0;
    let name = match test_type {
      TestType::StabilityOnly => "STABILITY",
      TestType::FullErasure => "FULL ERASURE",
      TestType::Repositioning => "REPOSITIONING",
    };
    self.log(format!("=== Starting {} test ===", name));
  }
}

fn setup(mut commands: Commands) {
  // Spawn camera centered on spawn area
  let center_x = (SPAWN_AREA.0 + SPAWN_AREA.2) / 2.0;
  let center_y = (SPAWN_AREA.1 + SPAWN_AREA.3) / 2.0;

  commands.spawn((
    Camera2d,
    StreamingCamera,
    Transform::from_xyz(center_x, center_y, 0.0),
    Projection::Orthographic(OrthographicProjection {
      near: -1000.0,
      far: 1000.0,
      scale: 1.0,
      viewport_origin: Vec2::new(0.5, 0.5),
      scaling_mode: ScalingMode::AutoMin {
        min_width: 320.0,
        min_height: 240.0,
      },
      area: Rect::default(),
    }),
  ));

  // Spawn pixel world
  commands.queue(SpawnPixelWorld::new(MaterialSeeder::new(42)));
}

/// Auto-starts a test if specified via command line
fn auto_start_test(mut state: ResMut<DebugState>, cli: Res<CliConfig>) {
  if !state.platform_ready || state.phase != TestPhase::Idle {
    return;
  }

  // Only auto-start once
  if state.test_type.is_some() {
    return;
  }

  match cli.auto_test {
    Some(1) => {
      state.log("Auto-starting stability test (--test 1)".to_string());
      state.start_test(TestType::StabilityOnly);
    }
    Some(2) => {
      state.log("Auto-starting erasure test (--test 2)".to_string());
      state.start_test(TestType::FullErasure);
    }
    Some(3) => {
      state.log("Auto-starting repositioning test (--test 3)".to_string());
      state.start_test(TestType::Repositioning);
    }
    _ => {}
  }
}

/// Exits the app if --exit flag was passed and test is complete
fn check_exit_condition(
  state: Res<DebugState>,
  cli: Res<CliConfig>,
  mut exit: EventWriter<bevy::app::AppExit>,
) {
  if cli.exit_on_complete && state.phase == TestPhase::Done && state.test_passed.is_some() {
    let code = if state.test_passed == Some(true) {
      bevy::app::AppExit::Success
    } else {
      bevy::app::AppExit::from_code(1)
    };
    exit.write(code);
  }
}

/// Clears the visible area and draws a stone platform once the world is seeded.
fn draw_platform(
  mut state: ResMut<DebugState>,
  mut worlds: Query<&mut PixelWorld>,
  gizmos: bevy_pixel_world::debug_shim::GizmosParam,
) {
  if state.platform_ready {
    return;
  }

  let Ok(mut world) = worlds.single_mut() else {
    return;
  };

  // Check if world is seeded by testing a pixel
  if world.get_pixel(WorldPos::new(0, 0)).is_none() {
    return;
  }

  // First, clear the entire visible area (spawn area + margins)
  let clear_x_min = SPAWN_AREA.0 as i64 - CLEAR_MARGIN;
  let clear_y_min = PLATFORM_Y - CLEAR_MARGIN;
  let clear_x_max = SPAWN_AREA.2 as i64 + CLEAR_MARGIN;
  let clear_y_max = SPAWN_AREA.3 as i64 + CLEAR_MARGIN;

  let clear_rect = WorldRect::new(
    clear_x_min,
    clear_y_min,
    (clear_x_max - clear_x_min) as u32,
    (clear_y_max - clear_y_min) as u32,
  );

  world.blit(clear_rect, |_| Some(Pixel::VOID), gizmos.get());

  // Draw stone platform (centered, wide)
  let stone = Pixel::new(material_ids::STONE, ColorIndex(100));
  let platform_x = (SPAWN_AREA.0 as i64 + SPAWN_AREA.2 as i64) / 2 - PLATFORM_WIDTH / 2;

  let rect = WorldRect::new(
    platform_x,
    PLATFORM_Y,
    PLATFORM_WIDTH as u32,
    PLATFORM_HEIGHT as u32,
  );

  world.blit(rect, |_| Some(stone), gizmos.get());

  state.platform_ready = true;
  state.log("Platform ready. Press 1 for stability test, 2 for erasure test".to_string());
}

fn camera_input(
  keys: Res<ButtonInput<KeyCode>>,
  mut camera: Query<&mut Transform, With<StreamingCamera>>,
  time: Res<Time>,
) {
  let mut direction = Vec2::ZERO;

  if keys.pressed(KeyCode::KeyW) {
    direction.y += 1.0;
  }
  if keys.pressed(KeyCode::KeyS) {
    direction.y -= 1.0;
  }
  if keys.pressed(KeyCode::KeyA) {
    direction.x -= 1.0;
  }
  if keys.pressed(KeyCode::KeyD) {
    direction.x += 1.0;
  }

  if direction != Vec2::ZERO {
    if let Ok(mut transform) = camera.single_mut() {
      let delta = direction.normalize() * CAMERA_SPEED * time.delta_secs();
      transform.translation.x += delta.x;
      transform.translation.y += delta.y;
    }
  }
}

fn manual_input(
  keys: Res<ButtonInput<KeyCode>>,
  mut state: ResMut<DebugState>,
  #[cfg(feature = "avian2d")] mut commands: Commands,
) {
  // Start tests
  if keys.just_pressed(KeyCode::Digit1) && state.phase == TestPhase::Idle && state.platform_ready {
    state.start_test(TestType::StabilityOnly);
  }
  if keys.just_pressed(KeyCode::Digit2) && state.phase == TestPhase::Idle && state.platform_ready {
    state.start_test(TestType::FullErasure);
  }
  if keys.just_pressed(KeyCode::Digit3) && state.phase == TestPhase::Idle && state.platform_ready {
    state.start_test(TestType::Repositioning);
  }

  // Manual spawn
  if keys.just_pressed(KeyCode::Space) && state.phase == TestPhase::Idle {
    #[cfg(feature = "avian2d")]
    {
      let mut rng = rand::thread_rng();
      let x = rng.gen_range(SPAWN_AREA.0..SPAWN_AREA.2);
      let y = rng.gen_range(SPAWN_AREA.1..SPAWN_AREA.3);
      let sprite = if rng.gen_bool(0.5) {
        "box.png"
      } else {
        "femur.png"
      };
      commands.queue(SpawnPixelBody::new(
        sprite,
        material_ids::WOOD,
        Vec2::new(x, y),
      ));
      state.bodies_spawned += 1;
      state.log(format!("Manual spawn at ({:.0}, {:.0})", x, y));
    }
    #[cfg(not(feature = "avian2d"))]
    state.log("Physics feature not enabled".to_string());
  }

  // Toggle manual erase
  if keys.just_pressed(KeyCode::KeyE) {
    state.manual_erase = !state.manual_erase;
    state.manual_erase_index = 0;
    let status = if state.manual_erase { "ON" } else { "OFF" };
    state.log(format!("Manual erase: {}", status));
  }
}

/// Main test phase runner
#[allow(clippy::too_many_arguments)]
fn run_test_phases(
  mut state: ResMut<DebugState>,
  time: Res<Time>,
  mut worlds: Query<&mut PixelWorld>,
  bodies: Query<&PixelBody>,
  mut camera: Query<&mut Transform, With<StreamingCamera>>,
  gizmos: bevy_pixel_world::debug_shim::GizmosParam,
  #[cfg(feature = "avian2d")] mut commands: Commands,
) {
  // Handle manual erase separately
  if state.manual_erase {
    state.manual_erase_timer.tick(time.delta());
    if state.manual_erase_timer.just_finished() {
      if let Ok(mut world) = worlds.single_mut() {
        let positions = get_erase_positions();
        if !positions.is_empty() {
          let (cx, cy) = positions[state.manual_erase_index % positions.len()];
          erase_circle(&mut world, cx, cy, BRUSH_RADIUS, gizmos.get());
          state.manual_erase_index += 1;
          if state.manual_erase_index >= positions.len() {
            state.manual_erase_index = 0;
          }
        }
      }
    }
  }

  match state.phase {
    TestPhase::Idle | TestPhase::Done => return,

    TestPhase::Spawning => {
      #[cfg(feature = "avian2d")]
      {
        state.spawn_timer.tick(time.delta());
        if state.spawn_timer.just_finished() && state.bodies_spawned < SPAWN_COUNT {
          let mut rng = rand::thread_rng();
          let x = rng.gen_range(SPAWN_AREA.0..SPAWN_AREA.2);
          let y = rng.gen_range(SPAWN_AREA.1..SPAWN_AREA.3);
          let sprite = if rng.gen_bool(0.5) {
            "box.png"
          } else {
            "femur.png"
          };
          commands.queue(SpawnPixelBody::new(
            sprite,
            material_ids::WOOD,
            Vec2::new(x, y),
          ));
          state.bodies_spawned += 1;
        }

        if state.bodies_spawned >= SPAWN_COUNT {
          state.phase = TestPhase::Settling;
          state.frame_counter = 0;
          state.log(format!("Spawned {} bodies, settling...", SPAWN_COUNT));
        }
      }
      #[cfg(not(feature = "avian2d"))]
      {
        state.log("Physics feature not enabled".to_string());
        state.phase = TestPhase::Done;
      }
    }

    TestPhase::Settling => {
      state.frame_counter += 1;
      if state.frame_counter >= SETTLE_FRAMES {
        let body_count = bodies.iter().count();
        let total_solid: usize = bodies.iter().map(|b| b.solid_count()).sum();
        state.log(format!(
          "Settled: {} bodies, {} total pixels",
          body_count, total_solid
        ));

        match state.test_type {
          Some(TestType::StabilityOnly) => {
            state.phase = TestPhase::Done;
            state.test_passed = Some(body_count == SPAWN_COUNT);
            if body_count == SPAWN_COUNT {
              state.log("=== STABILITY TEST PASSED ===".to_string());
            } else {
              state.log(format!(
                "=== STABILITY TEST FAILED: expected {} bodies, got {} ===",
                SPAWN_COUNT, body_count
              ));
            }
          }
          Some(TestType::FullErasure) => {
            state.phase = TestPhase::Erasing;
            state.frame_counter = 0;
            // Start sweep ABOVE platform to preserve it
            state.brush_x = SPAWN_AREA.0 as i64 - CLEAR_MARGIN;
            state.brush_y = PLATFORM_Y + PLATFORM_HEIGHT + BRUSH_RADIUS + 1;
            state.log("Starting brush erasure sweep...".to_string());
          }
          Some(TestType::Repositioning) => {
            // Record counts and camera position before scrolling
            state.pre_scroll_body_count = body_count;
            state.pre_scroll_pixel_count = total_solid;
            if let Ok(cam_transform) = camera.single() {
              state.original_camera_y = cam_transform.translation.y;
            }
            state.phase = TestPhase::ScrollingUp;
            state.frame_counter = 0;
            state.log(format!(
              "Bodies settled: {} bodies, {} pixels. Starting scroll up...",
              body_count, total_solid
            ));
          }
          None => {
            state.phase = TestPhase::Done;
          }
        }
      }
    }

    TestPhase::Erasing => {
      let Ok(mut world) = worlds.single_mut() else {
        return;
      };

      // Brush sweep parameters - ONLY erase above platform to preserve it
      // Bodies rest on platform at Y = PLATFORM_Y + PLATFORM_HEIGHT = 60
      // Brush radius is 30, so brush center at Y=90 covers Y=60-120
      // Start at Y=91 to ensure we don't touch platform at Y=60
      let x_min = SPAWN_AREA.0 as i64 - CLEAR_MARGIN;
      let x_max = SPAWN_AREA.2 as i64 + CLEAR_MARGIN;
      let y_min = PLATFORM_Y + PLATFORM_HEIGHT + BRUSH_RADIUS + 1; // 61 pixels above platform
      let y_max = SPAWN_AREA.3 as i64 + CLEAR_MARGIN;
      // Use smaller step for more thorough coverage
      let step = BRUSH_RADIUS / 2;

      // Erase at current position
      erase_circle(
        &mut world,
        state.brush_x,
        state.brush_y,
        BRUSH_RADIUS,
        gizmos.get(),
      );

      // Move to next position
      state.brush_x += step;
      if state.brush_x > x_max {
        state.brush_x = x_min;
        state.brush_y += step;
      }

      // Check if sweep is complete
      if state.brush_y > y_max {
        // Do multiple passes to be thorough
        state.frame_counter += 1;
        if state.frame_counter >= 4 {
          state.phase = TestPhase::Verifying;
          state.frame_counter = 0;
          state.verify_body_counts.clear();
          state.verify_pixel_counts.clear();
          state.log("Erasure complete, verifying...".to_string());
        } else {
          // Reset for another pass - start above platform, lower by 2 pixels each pass
          state.brush_x = SPAWN_AREA.0 as i64 - CLEAR_MARGIN;
          // Lower the brush by 2 pixels each pass to catch pixels at the edge
          let pass_offset = (state.frame_counter as i64) * 2;
          let new_y = PLATFORM_Y + PLATFORM_HEIGHT + BRUSH_RADIUS + 1 - pass_offset;
          state.brush_y = new_y;
          let pass = state.frame_counter + 1;
          state.log(format!(
            "Starting erasure pass {} (y_start={})...",
            pass, new_y
          ));
        }
      }
    }

    TestPhase::ScrollingUp => {
      let Ok(mut cam_transform) = camera.single_mut() else {
        return;
      };

      let target_y = state.original_camera_y + REPOSITION_DISTANCE;
      let delta = SCROLL_SPEED * time.delta_secs();

      if cam_transform.translation.y < target_y {
        cam_transform.translation.y = (cam_transform.translation.y + delta).min(target_y);
      } else {
        state.phase = TestPhase::WaitingUp;
        state.frame_counter = 0;
        state.log(format!(
          "Reached top position (y={}). Waiting for chunk repositioning...",
          cam_transform.translation.y
        ));
      }
    }

    TestPhase::WaitingUp => {
      state.frame_counter += 1;
      if state.frame_counter >= WAIT_UP_FRAMES {
        state.phase = TestPhase::ScrollingDown;
        state.frame_counter = 0;
        state.log("Scrolling back down...".to_string());
      }
    }

    TestPhase::ScrollingDown => {
      let Ok(mut cam_transform) = camera.single_mut() else {
        return;
      };

      let target_y = state.original_camera_y;
      let delta = SCROLL_SPEED * time.delta_secs();

      if cam_transform.translation.y > target_y {
        cam_transform.translation.y = (cam_transform.translation.y - delta).max(target_y);
      } else {
        state.phase = TestPhase::Verifying;
        state.frame_counter = 0;
        state.verify_body_counts.clear();
        state.verify_pixel_counts.clear();
        state.log("Returned to original position. Verifying...".to_string());
      }
    }

    TestPhase::Verifying => {
      let body_count = bodies.iter().count();
      let world_body_pixels = count_body_pixels(&worlds);
      let total_solid: usize = bodies.iter().map(|b| b.solid_count()).sum();

      state.verify_body_counts.push(body_count);
      state.verify_pixel_counts.push(world_body_pixels);
      state.frame_counter += 1;

      // Log periodic updates
      if state.frame_counter % 10 == 0 {
        let frame = state.frame_counter;
        state.log(format!(
          "Verify frame {}: {} bodies, {} PIXEL_BODY flags, {} solid pixels",
          frame, body_count, world_body_pixels, total_solid
        ));
      }

      if state.frame_counter >= VERIFY_FRAMES {
        state.phase = TestPhase::Done;

        match state.test_type {
          Some(TestType::FullErasure) => {
            // Check results: all counts should be 0
            let all_bodies_zero = state.verify_body_counts.iter().all(|&c| c == 0);
            let all_pixels_zero = state.verify_pixel_counts.iter().all(|&c| c == 0);

            let max_bodies = *state.verify_body_counts.iter().max().unwrap_or(&0);
            let max_pixels = *state.verify_pixel_counts.iter().max().unwrap_or(&0);

            if all_bodies_zero && all_pixels_zero {
              state.test_passed = Some(true);
              state.log("=== ERASURE TEST PASSED ===".to_string());
              state.log("All bodies removed, no ghost pixels".to_string());
            } else {
              state.test_passed = Some(false);
              state.log("=== ERASURE TEST FAILED ===".to_string());
              state.log(format!(
                "Remaining: max {} bodies, max {} PIXEL_BODY flags",
                max_bodies, max_pixels
              ));
            }
          }
          Some(TestType::Repositioning) => {
            // Check that body count and pixel count match pre-scroll values
            let final_body_count = *state.verify_body_counts.last().unwrap_or(&0);
            let final_world_pixels = *state.verify_pixel_counts.last().unwrap_or(&0);
            let expected_body_count = state.pre_scroll_body_count;

            let body_count_match = final_body_count == expected_body_count;
            // World pixels should roughly match total solid pixels from bodies
            // (some variance is acceptable due to overlap at chunk boundaries)
            let pixel_variance = (final_world_pixels as i64 - total_solid as i64).abs();
            let pixel_count_reasonable = pixel_variance < (total_solid as i64 / 10).max(50);

            state.log(format!(
              "Final: {} bodies (expected {}), {} world pixels, {} body pixels",
              final_body_count, expected_body_count, final_world_pixels, total_solid
            ));

            if body_count_match && pixel_count_reasonable {
              state.test_passed = Some(true);
              state.log("=== REPOSITIONING TEST PASSED ===".to_string());
              state.log("No pixel duplication after chunk repositioning".to_string());
            } else {
              state.test_passed = Some(false);
              state.log("=== REPOSITIONING TEST FAILED ===".to_string());
              if !body_count_match {
                state.log(format!(
                  "Body count mismatch: {} vs expected {}",
                  final_body_count, expected_body_count
                ));
              }
              if !pixel_count_reasonable {
                state.log(format!(
                  "Pixel count variance too high: world has {}, bodies have {} (variance: {})",
                  final_world_pixels, total_solid, pixel_variance
                ));
              }
            }
          }
          _ => {}
        }
      }
    }
  }
}

/// Erases a circle at the given position
fn erase_circle(
  world: &mut PixelWorld,
  center_x: i64,
  center_y: i64,
  radius: i64,
  gizmos: bevy_pixel_world::debug_shim::DebugGizmos<'_>,
) {
  let rect = WorldRect::centered(center_x, center_y, radius as u32);
  let radius_sq = radius * radius;

  world.blit(
    rect,
    |frag| {
      let dx = frag.x - center_x;
      let dy = frag.y - center_y;
      if dx * dx + dy * dy <= radius_sq {
        Some(Pixel::VOID)
      } else {
        None
      }
    },
    gizmos,
  );
}

/// Returns grid positions for manual erase sweep
fn get_erase_positions() -> Vec<(i64, i64)> {
  let mut positions = Vec::new();
  let step = 15i64;
  for y in ((SPAWN_AREA.1 as i64 - 20)..(SPAWN_AREA.3 as i64 + 20)).step_by(step as usize) {
    for x in ((SPAWN_AREA.0 as i64 - 20)..(SPAWN_AREA.2 as i64 + 20)).step_by(step as usize) {
      positions.push((x, y));
    }
  }
  positions
}

/// Counts pixels with PIXEL_BODY flag in the test area
fn count_body_pixels(worlds: &Query<&mut PixelWorld>) -> usize {
  let Ok(world) = worlds.single() else {
    return 0;
  };

  let mut count = 0;
  for y in (PLATFORM_Y - 10)..(SPAWN_AREA.3 as i64 + CLEAR_MARGIN) {
    for x in (SPAWN_AREA.0 as i64 - CLEAR_MARGIN)..(SPAWN_AREA.2 as i64 + CLEAR_MARGIN) {
      if let Some(pixel) = world.get_pixel(WorldPos::new(x, y)) {
        if pixel.flags.contains(PixelFlags::PIXEL_BODY) {
          count += 1;
        }
      }
    }
  }
  count
}

fn diagnostic_ui(
  mut contexts: EguiContexts,
  state: Res<DebugState>,
  bodies: Query<(
    Entity,
    &PixelBody,
    &GlobalTransform,
    Option<&LastBlitTransform>,
  )>,
  worlds: Query<&mut PixelWorld>,
) {
  let Ok(ctx) = contexts.ctx_mut() else {
    return;
  };

  // Collect body diagnostics
  let body_count = bodies.iter().count();
  let total_solid: usize = bodies.iter().map(|(_, b, _, _)| b.solid_count()).sum();
  let world_body_pixels = count_body_pixels(&worlds);

  egui::SidePanel::left("debug_panel")
    .resizable(true)
    .default_width(320.0)
    .show(ctx, |ui| {
      ui.heading("Pixel Body Erasure Test");

      ui.separator();
      ui.label("Controls:");
      ui.label("  1 - Run stability test (spawn only)");
      ui.label("  2 - Run erasure test (spawn + erase)");
      ui.label("  3 - Run repositioning test (spawn + scroll)");
      ui.label("  Space - Manual spawn");
      ui.label("  E - Toggle manual erase sweep");
      ui.label("  WASD - Move camera");

      ui.separator();

      // Phase indicator with color
      let (phase_text, phase_color) = match state.phase {
        TestPhase::Idle => ("IDLE", egui::Color32::GRAY),
        TestPhase::Spawning => ("SPAWNING", egui::Color32::YELLOW),
        TestPhase::Settling => ("SETTLING", egui::Color32::LIGHT_BLUE),
        TestPhase::Erasing => ("ERASING", egui::Color32::ORANGE),
        TestPhase::ScrollingUp => ("SCROLLING UP", egui::Color32::KHAKI),
        TestPhase::WaitingUp => ("WAITING UP", egui::Color32::LIGHT_BLUE),
        TestPhase::ScrollingDown => ("SCROLLING DOWN", egui::Color32::KHAKI),
        TestPhase::Verifying => ("VERIFYING", egui::Color32::LIGHT_BLUE),
        TestPhase::Done => {
          if state.test_passed == Some(true) {
            ("PASSED", egui::Color32::GREEN)
          } else if state.test_passed == Some(false) {
            ("FAILED", egui::Color32::RED)
          } else {
            ("DONE", egui::Color32::GRAY)
          }
        }
      };
      ui.horizontal(|ui| {
        ui.label("Phase:");
        ui.colored_label(phase_color, phase_text);
      });

      if let Some(test_type) = state.test_type {
        ui.label(format!("Test: {:?}", test_type));
      }

      ui.separator();
      ui.heading("Status");

      ui.label(format!("Bodies spawned: {}", state.bodies_spawned));
      ui.label(format!("Bodies alive: {}", body_count));
      ui.label(format!("Total solid pixels: {}", total_solid));
      ui.label(format!("World PIXEL_BODY flags: {}", world_body_pixels));

      if matches!(
        state.phase,
        TestPhase::Settling | TestPhase::Verifying | TestPhase::WaitingUp
      ) {
        ui.label(format!("Frame: {}", state.frame_counter));
      }

      ui.separator();
      ui.heading("Bodies");

      egui::ScrollArea::vertical()
        .id_salt("bodies_scroll")
        .max_height(150.0)
        .show(ui, |ui| {
          for (entity, body, transform, last_blit) in bodies.iter() {
            let pos = transform.translation();
            let solid = body.solid_count();
            let has_blit = last_blit.and_then(|b| b.transform.as_ref()).is_some();

            ui.horizontal(|ui| {
              ui.label(format!(
                "{:?}: ({:.0},{:.0}) s={} b={}",
                entity,
                pos.x,
                pos.y,
                solid,
                if has_blit { "Y" } else { "N" }
              ));
            });
          }
        });

      ui.separator();
      ui.heading("Log");

      egui::ScrollArea::vertical()
        .id_salt("log_scroll")
        .max_height(200.0)
        .stick_to_bottom(true)
        .show(ui, |ui| {
          for line in &state.log {
            // Color-code important messages
            let color = if line.contains("PASSED") {
              egui::Color32::GREEN
            } else if line.contains("FAILED") {
              egui::Color32::RED
            } else if line.starts_with("===") {
              egui::Color32::WHITE
            } else {
              egui::Color32::LIGHT_GRAY
            };
            ui.colored_label(color, line);
          }
        });
    });
}
