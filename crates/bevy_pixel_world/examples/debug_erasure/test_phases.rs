use bevy::prelude::*;
#[cfg(feature = "avian2d")]
use bevy_pixel_world::SpawnPixelBody;
use bevy_pixel_world::pixel_body::PixelBody;
use bevy_pixel_world::{
  Pixel, PixelFlags, PixelWorld, StreamingCamera, WorldPos, WorldRect, material_ids,
};
use rand::Rng;

use crate::state::*;

/// Main test phase runner
#[allow(clippy::too_many_arguments)]
pub fn run_test_phases(
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
            state.brush_x = SPAWN_AREA.0 as i64 - CLEAR_MARGIN;
            state.brush_y = PLATFORM_Y + PLATFORM_HEIGHT + BRUSH_RADIUS + 1;
            state.log("Starting brush erasure sweep...".to_string());
          }
          Some(TestType::Repositioning) => {
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

      let x_min = SPAWN_AREA.0 as i64 - CLEAR_MARGIN;
      let x_max = SPAWN_AREA.2 as i64 + CLEAR_MARGIN;
      let y_min = PLATFORM_Y + PLATFORM_HEIGHT + BRUSH_RADIUS + 1;
      let y_max = SPAWN_AREA.3 as i64 + CLEAR_MARGIN;
      let step = BRUSH_RADIUS / 2;

      erase_circle(
        &mut world,
        state.brush_x,
        state.brush_y,
        BRUSH_RADIUS,
        gizmos.get(),
      );

      state.brush_x += step;
      if state.brush_x > x_max {
        state.brush_x = x_min;
        state.brush_y += step;
      }

      if state.brush_y > y_max {
        state.frame_counter += 1;
        if state.frame_counter >= 4 {
          state.phase = TestPhase::Verifying;
          state.frame_counter = 0;
          state.verify_body_counts.clear();
          state.verify_pixel_counts.clear();
          state.log("Erasure complete, verifying...".to_string());
        } else {
          state.brush_x = SPAWN_AREA.0 as i64 - CLEAR_MARGIN;
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
            let final_body_count = *state.verify_body_counts.last().unwrap_or(&0);
            let final_world_pixels = *state.verify_pixel_counts.last().unwrap_or(&0);
            let expected_body_count = state.pre_scroll_body_count;

            let body_count_match = final_body_count == expected_body_count;
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
pub fn count_body_pixels(worlds: &Query<&mut PixelWorld>) -> usize {
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
