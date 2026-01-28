//! Radial ray-cast blast primitive for `PixelWorld`.
//!
//! The `blast` method casts rays outward from a center point, invoking a
//! caller-supplied callback for each hit pixel. The callback controls
//! energy consumption and pixel replacement, making the ray-march
//! infrastructure reusable for different blast behaviors.

use bevy::math::Vec2;

use super::PixelWorld;
use crate::coords::WorldPos;
use crate::debug_shim::DebugGizmos;
use crate::pixel::Pixel;

/// Parameters for a radial blast.
pub struct BlastParams {
  /// World-space center of the blast.
  pub center: Vec2,
  /// Initial energy per ray.
  pub strength: f32,
  /// Maximum ray length in pixels.
  pub max_radius: f32,
  /// Radius for heat injection (smooth spherical falloff).
  pub heat_radius: f32,
}

/// What the blast callback wants to do with a hit pixel.
pub enum BlastHit {
  /// Skip this pixel, ray continues without energy cost.
  Skip,
  /// Replace pixel and consume energy. Ray stops if energy drops to zero.
  Hit { pixel: Pixel, cost: f32 },
  /// Stop the ray immediately.
  Stop,
}

impl PixelWorld {
  /// Radial ray-cast blast from `params.center`.
  ///
  /// For each non-void pixel hit by a ray, calls `f(pixel, pos)`.
  /// The callback returns a [`BlastHit`] controlling energy consumption
  /// and pixel replacement. After all rays, awakens boundary pixels and
  /// injects heat over `heat_radius`.
  pub fn blast<F>(&mut self, params: &BlastParams, f: F)
  where
    F: Fn(&Pixel, WorldPos) -> BlastHit,
  {
    let center = params.center;
    let radius = params.max_radius;
    let r = radius as i32;

    // Number of rays: circumference ensures every edge pixel is hit
    let num_rays = (2.0 * std::f32::consts::PI * radius).ceil() as usize;

    for ray_idx in 0..num_rays {
      let angle = 2.0 * std::f32::consts::PI * ray_idx as f32 / num_rays as f32;
      let dir_x = angle.cos();
      let dir_y = angle.sin();
      let mut remaining = params.strength;

      for step in 0..=r {
        let wx = center.x as i64 + (dir_x * step as f32).round() as i64;
        let wy = center.y as i64 + (dir_y * step as f32).round() as i64;
        let pos = WorldPos::new(wx, wy);

        let Some(pixel) = self.get_pixel(pos).copied() else {
          break; // unloaded chunk
        };

        if pixel.is_void() {
          continue;
        }

        match f(&pixel, pos) {
          BlastHit::Skip => continue,
          BlastHit::Stop => break,
          BlastHit::Hit {
            pixel: new_pixel,
            cost,
          } => {
            remaining -= cost;
            self.set_pixel(pos, new_pixel, DebugGizmos::none());
            self.mark_pixel_sim_dirty(pos);
            if remaining <= 0.0 {
              break;
            }
          }
        }
      }
    }

    // Awaken boundary pixels so exposed material falls/flows
    for ray_idx in 0..(2.0 * std::f32::consts::PI * (radius + 2.0)).ceil() as usize {
      let angle = 2.0 * std::f32::consts::PI * ray_idx as f32
        / (2.0 * std::f32::consts::PI * (radius + 2.0)).ceil();
      for dr in [-1i32, 0, 1] {
        let step = (radius as i32 + dr).max(0);
        let wx = center.x as i64 + (angle.cos() * step as f32).round() as i64;
        let wy = center.y as i64 + (angle.sin() * step as f32).round() as i64;
        self.mark_pixel_sim_dirty(WorldPos::new(wx, wy));
      }
    }

    // Inject heat with smooth spherical falloff
    let heat_radius = params.heat_radius;
    let hr = heat_radius as i32;
    let hr_sq = heat_radius * heat_radius;
    for dy in -hr..=hr {
      for dx in -hr..=hr {
        let dist_sq = (dx * dx + dy * dy) as f32;
        if dist_sq > hr_sq {
          continue;
        }
        let t = (dist_sq / hr_sq).sqrt();
        let heat = ((1.0 - t) * 255.0) as u8;
        if heat == 0 {
          continue;
        }
        let pos = WorldPos::new(center.x as i64 + dx as i64, center.y as i64 + dy as i64);
        self.set_heat_at(pos, heat);
      }
    }
  }
}
