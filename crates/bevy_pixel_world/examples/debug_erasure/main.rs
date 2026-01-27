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

mod state;
mod test_phases;
mod ui;

use bevy::camera::ScalingMode;
use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
#[cfg(feature = "avian2d")]
use bevy_pixel_world::SpawnPixelBody;
use bevy_pixel_world::diagnostics::DiagnosticsPlugin;
use bevy_pixel_world::{
  MaterialSeeder, PersistenceConfig, Pixel, PixelWorld, PixelWorldPlugin, SpawnPixelWorld,
  StreamingCamera, WorldPos, WorldRect, material_ids,
};
use rand::Rng;
use state::*;

fn main() {
  let cli_config = parse_args();

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
    .add_plugins(bevy_pixel_world::PixelBodiesPlugin)
    .add_plugins(EguiPlugin::default())
    .insert_resource(cli_config)
    .init_resource::<DebugState>()
    .add_systems(Startup, setup)
    .add_systems(EguiPrimaryContextPass, ui::diagnostic_ui)
    .add_systems(
      Update,
      (
        draw_platform,
        auto_start_test,
        camera_input,
        manual_input,
        test_phases::run_test_phases,
        check_exit_condition,
      ),
    );

  app.add_plugins(DiagnosticsPlugin);

  #[cfg(feature = "avian2d")]
  {
    app.add_plugins(avian2d::prelude::PhysicsPlugins::default());
    app.insert_resource(avian2d::prelude::Gravity(Vec2::new(0.0, -500.0)));
  }

  app.run();
}

fn setup(mut commands: Commands) {
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

  commands.queue(SpawnPixelWorld::new(MaterialSeeder::new(42)));
}

fn auto_start_test(mut state: ResMut<DebugState>, cli: Res<CliConfig>) {
  if !state.platform_ready || state.phase != TestPhase::Idle {
    return;
  }

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

  if world.get_pixel(WorldPos::new(0, 0)).is_none() {
    return;
  }

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

  let stone = Pixel::new(material_ids::STONE, bevy_pixel_world::ColorIndex(100));
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
  if keys.just_pressed(KeyCode::Digit1) && state.phase == TestPhase::Idle && state.platform_ready {
    state.start_test(TestType::StabilityOnly);
  }
  if keys.just_pressed(KeyCode::Digit2) && state.phase == TestPhase::Idle && state.platform_ready {
    state.start_test(TestType::FullErasure);
  }
  if keys.just_pressed(KeyCode::Digit3) && state.phase == TestPhase::Idle && state.platform_ready {
    state.start_test(TestType::Repositioning);
  }

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

  if keys.just_pressed(KeyCode::KeyE) {
    state.manual_erase = !state.manual_erase;
    state.manual_erase_index = 0;
    let status = if state.manual_erase { "ON" } else { "OFF" };
    state.log(format!("Manual erase: {}", status));
  }
}
