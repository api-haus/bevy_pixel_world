use bevy::prelude::*;

pub const CAMERA_SPEED: f32 = 300.0;
pub const SPAWN_AREA: (f32, f32, f32, f32) = (60.0, 80.0, 140.0, 160.0);
pub const BRUSH_RADIUS: i64 = 30;
pub const PLATFORM_Y: i64 = 20;
pub const PLATFORM_WIDTH: i64 = 400;
pub const PLATFORM_HEIGHT: i64 = 40;
pub const CLEAR_MARGIN: i64 = 100;

pub const SPAWN_COUNT: usize = 15;
pub const SETTLE_FRAMES: usize = 120;
pub const VERIFY_FRAMES: usize = 60;

// Repositioning test constants
pub const CHUNK_SIZE_PX: f32 = 64.0;
pub const REPOSITION_DISTANCE: f32 = CHUNK_SIZE_PX * 5.0;
pub const SCROLL_SPEED: f32 = 200.0;
pub const WAIT_UP_FRAMES: usize = 30;

/// Command line configuration
#[derive(Resource, Default)]
pub struct CliConfig {
  /// Test to auto-start (1 = stability, 2 = erasure)
  pub auto_test: Option<u8>,
  /// Exit after test completes
  pub exit_on_complete: bool,
}

pub fn parse_args() -> CliConfig {
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

/// Current phase of the automated test.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TestPhase {
  Idle,
  Spawning,
  Settling,
  Erasing,
  ScrollingUp,
  WaitingUp,
  ScrollingDown,
  Verifying,
  Done,
}

/// Type of test being run
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TestType {
  StabilityOnly,
  FullErasure,
  Repositioning,
}

#[derive(Resource)]
pub struct DebugState {
  pub phase: TestPhase,
  pub test_type: Option<TestType>,
  pub bodies_spawned: usize,
  pub spawn_timer: Timer,
  pub frame_counter: usize,
  pub brush_x: i64,
  pub brush_y: i64,
  pub log: Vec<String>,
  pub max_log: usize,
  pub platform_ready: bool,
  pub verify_body_counts: Vec<usize>,
  pub verify_pixel_counts: Vec<usize>,
  pub test_passed: Option<bool>,
  pub manual_erase: bool,
  pub manual_erase_index: usize,
  pub manual_erase_timer: Timer,
  pub original_camera_y: f32,
  pub pre_scroll_body_count: usize,
  pub pre_scroll_pixel_count: usize,
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
  pub fn log(&mut self, msg: String) {
    info!("{}", msg);
    self.log.push(msg);
    if self.log.len() > self.max_log {
      self.log.remove(0);
    }
  }

  pub fn start_test(&mut self, test_type: TestType) {
    self.phase = TestPhase::Spawning;
    self.test_type = Some(test_type);
    self.bodies_spawned = 0;
    self.frame_counter = 0;
    self.brush_x = SPAWN_AREA.0 as i64 - CLEAR_MARGIN;
    self.brush_y = PLATFORM_Y + PLATFORM_HEIGHT + BRUSH_RADIUS;
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
