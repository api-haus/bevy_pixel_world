//! Systems for pixel camera snapping and state synchronization.

use bevy::image::ImageSampler;
use bevy::prelude::*;

use super::components::LogicalCameraPosition;
use super::config::PixelCameraConfig;
use super::material::PixelBlitMaterial;
use super::setup::{PixelBlitCamera, PixelBlitQuad, PixelSceneCamera};
use super::state::PixelCameraState;

/// System: Stores the logical camera position before snapping.
///
/// Runs after camera_follow to capture the smooth camera position.
/// This position is used by streaming systems to avoid chunk pop-in.
pub fn pixel_camera_store_logical(
  mut camera_query: Query<(&Transform, &mut LogicalCameraPosition), With<PixelSceneCamera>>,
) {
  for (transform, mut logical_pos) in camera_query.iter_mut() {
    logical_pos.0 = Vec2::new(transform.translation.x, transform.translation.y);
  }
}

/// System: Snaps the camera to the pixel grid and calculates UV offset.
///
/// After snapping, the subpixel delta is stored in PixelCameraState for
/// the blit shader to use as a UV offset.
pub fn pixel_camera_snap(
  config: Res<PixelCameraConfig>,
  mut state: ResMut<PixelCameraState>,
  mut camera_query: Query<&mut Transform, With<PixelSceneCamera>>,
) {
  if !state.initialized {
    return;
  }

  let pixel_world_size = state.pixel_world_size;
  if pixel_world_size <= 0.0 {
    return;
  }

  for mut transform in camera_query.iter_mut() {
    let logical_pos = Vec2::new(transform.translation.x, transform.translation.y);

    // Snap to nearest pixel in world space
    // Convert to integer pixel coords to avoid float precision errors
    let pixel_x = (logical_pos.x / pixel_world_size).round() as i32;
    let pixel_y = (logical_pos.y / pixel_world_size).round() as i32;
    let snapped_x = pixel_x as f32 * pixel_world_size;
    let snapped_y = pixel_y as f32 * pixel_world_size;

    // Calculate subpixel delta (always in range [-0.5, 0.5] * pixel_world_size)
    let delta_world = logical_pos - Vec2::new(snapped_x, snapped_y);

    // Convert to UV offset for blit shader
    // Target size includes margin, so we need viewport dimensions
    let margin = config.margin;
    let viewport_width = state.target_size.x - margin * 2;
    let viewport_height = state.target_size.y - margin * 2;

    if viewport_width > 0 && viewport_height > 0 {
      // Delta in pixels
      let delta_pixels = delta_world / pixel_world_size;

      // Delta in UV space (relative to total target size)
      // Both axes have same sign because:
      // - Higher delta means camera should be further right/up
      // - To show that view, sample from further right/down in texture
      // - UV coordinates: U+ is right, V+ is down
      let delta_uv = Vec2::new(
        delta_pixels.x / state.target_size.x as f32,
        delta_pixels.y / state.target_size.y as f32,
      );

      state.subpixel_offset_uv = if config.subpixel_smoothing {
        delta_uv
      } else {
        Vec2::ZERO
      };
    }

    // Update camera transform to snapped position
    transform.translation.x = snapped_x;
    transform.translation.y = snapped_y;
  }
}

/// System: Syncs pixel camera state to the blit material.
pub fn pixel_camera_sync_state(
  state: Res<PixelCameraState>,
  blit_quad_query: Query<&MeshMaterial2d<PixelBlitMaterial>, With<PixelBlitQuad>>,
  mut blit_materials: ResMut<Assets<PixelBlitMaterial>>,
) {
  if !state.initialized {
    return;
  }

  for material_handle in blit_quad_query.iter() {
    if let Some(material) = blit_materials.get_mut(&material_handle.0) {
      material.uniforms.subpixel_offset = state.subpixel_offset_uv;
    }
  }
}

/// System: Handles viewport resize by recreating the render target.
#[allow(clippy::too_many_arguments)]
pub fn pixel_camera_handle_resize(
  config: Res<PixelCameraConfig>,
  mut state: ResMut<PixelCameraState>,
  mut images: ResMut<Assets<Image>>,
  mut blit_materials: ResMut<Assets<PixelBlitMaterial>>,
  mut camera_query: Query<&mut Projection, With<PixelSceneCamera>>,
  blit_quad_query: Query<&MeshMaterial2d<PixelBlitMaterial>, With<PixelBlitQuad>>,
  windows: Query<&Window>,
  mut last_window_size: Local<(u32, u32)>,
  mut skip_first_frame: Local<bool>,
) {
  if !state.initialized {
    return;
  }

  // Skip the first frame after initialization to let the GPU catch up
  if !*skip_first_frame {
    *skip_first_frame = true;
    return;
  }

  let Ok(window) = windows.single() else {
    return;
  };

  let window_width = window.physical_width();
  let window_height = window.physical_height();

  // Skip if size hasn't changed
  if *last_window_size == (window_width, window_height) {
    return;
  }
  *last_window_size = (window_width, window_height);

  if window_width == 0 || window_height == 0 {
    return;
  }

  // Calculate target dimensions based on pixel size mode and window aspect ratio
  let aspect_ratio = window_width as f32 / window_height as f32;

  let (target_width, target_height, pixel_world_size) = match config.pixel_size_mode {
    super::config::PixelSizeMode::FixedVerticalResolution(height) => {
      // Use the stored pixel_world_size (set during initial setup)
      let target_height = height;
      let target_width = (height as f32 * aspect_ratio).ceil() as u32;
      (target_width, target_height, state.pixel_world_size)
    }
    super::config::PixelSizeMode::WorldSpacePixelSize(size) => {
      // Fixed pixel size - derive target from current projection area
      let pixel_world_size = size;
      // Use the current target height as reference (maintains view consistency)
      let current_target_height = state.target_size.y - config.margin * 2;
      let target_height = current_target_height.max(1);
      let target_width = (target_height as f32 * aspect_ratio).ceil() as u32;
      (target_width, target_height, pixel_world_size)
    }
  };

  let margin = config.margin;
  let total_width = target_width + margin * 2;
  let total_height = target_height + margin * 2;

  // Skip if target size hasn't changed
  if state.target_size == UVec2::new(total_width, total_height) {
    return;
  }

  info!(
    "Pixel camera resize: {}x{} target ({}px margin)",
    target_width, target_height, margin
  );

  // Resize the existing render target
  if let Some(image) = images.get_mut(&state.render_target) {
    let size = bevy::render::render_resource::Extent3d {
      width: total_width,
      height: total_height,
      depth_or_array_layers: 1,
    };
    image.resize(size);
    // Ensure sampler remains nearest-neighbor after resize
    image.sampler = ImageSampler::nearest();
  }

  // Update scene camera projection to exactly match new render target
  let half_width = total_width as f32 * pixel_world_size / 2.0;
  let half_height = total_height as f32 * pixel_world_size / 2.0;
  for mut projection in camera_query.iter_mut() {
    *projection = Projection::Orthographic(OrthographicProjection {
      near: -1000.0,
      far: 1000.0,
      scale: 1.0,
      viewport_origin: Vec2::new(0.5, 0.5),
      scaling_mode: bevy::camera::ScalingMode::Fixed {
        width: half_width * 2.0,
        height: half_height * 2.0,
      },
      area: Rect::default(),
    });
  }

  // Update state
  state.target_size = UVec2::new(total_width, total_height);
  state.pixel_world_size = pixel_world_size;

  // Update blit material viewport rect
  let viewport_rect = Vec4::new(
    margin as f32 / total_width as f32,
    margin as f32 / total_height as f32,
    target_width as f32 / total_width as f32,
    target_height as f32 / total_height as f32,
  );

  for material_handle in blit_quad_query.iter() {
    if let Some(material) = blit_materials.get_mut(&material_handle.0) {
      material.uniforms.viewport_rect = viewport_rect;
    }
  }
}

/// System: Configures egui to render at full resolution on the blit camera.
///
/// This moves the EguiContext from the scene camera (low-res) to the blit
/// camera (full-res), ensuring UI elements are sharp and not pixelated.
pub fn configure_egui_camera(
  mut commands: Commands,
  config: Res<PixelCameraConfig>,
  scene_camera: Query<Entity, With<PixelSceneCamera>>,
  blit_camera: Query<Entity, (With<PixelBlitCamera>, Without<bevy_egui::EguiContext>)>,
  egui_on_scene: Query<Entity, (With<PixelSceneCamera>, With<bevy_egui::EguiContext>)>,
) {
  if !config.egui_full_resolution {
    return;
  }

  // Remove EguiContext and PrimaryEguiContext from scene camera if present
  for entity in egui_on_scene.iter() {
    commands
      .entity(entity)
      .remove::<bevy_egui::EguiContext>()
      .remove::<bevy_egui::PrimaryEguiContext>();
  }

  // Add EguiContext and PrimaryEguiContext to blit camera if not present
  for entity in blit_camera.iter() {
    commands.entity(entity).insert((
      bevy_egui::EguiContext::default(),
      bevy_egui::PrimaryEguiContext,
    ));
  }

  // Also ensure scene camera doesn't have egui even if it was just added
  for entity in scene_camera.iter() {
    commands
      .entity(entity)
      .remove::<bevy_egui::EguiContext>()
      .remove::<bevy_egui::PrimaryEguiContext>();
  }
}
