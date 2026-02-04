//! Setup systems for pixel-perfect camera rendering.

use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{
  Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};

/// Layer for full-resolution sprites that bypass pixel snapping.
pub const FULLRES_SPRITE_LAYER: usize = 30;

/// Layer reserved for the blit system (quad and camera).
pub const BLIT_LAYER: usize = 31;

use super::components::{LogicalCameraPosition, PixelCamera};
use super::config::{PixelCameraConfig, PixelSizeMode};
use super::material::{PixelBlitMaterial, PixelBlitUniforms};
use super::state::PixelCameraState;

/// Marker for the scene camera that renders to the low-res target.
#[derive(Component)]
pub struct PixelSceneCamera;

/// Marker for the blit quad entity.
#[derive(Component)]
pub struct PixelBlitQuad;

/// Marker for the blit camera that renders to the screen.
#[derive(Component)]
pub struct PixelBlitCamera;

/// Marker for the full-resolution sprite camera.
#[derive(Component)]
pub struct PixelFullresCamera;

/// System: Sets up the pixel camera rendering infrastructure.
///
/// Creates:
/// - Low-resolution render target image
/// - Scene camera (renders game content to low-res target, layer 1)
/// - Blit quad with material (renders to screen, layer 0)
#[allow(clippy::too_many_arguments)]
pub fn setup_pixel_camera(
  mut commands: Commands,
  config: Res<PixelCameraConfig>,
  mut state: ResMut<PixelCameraState>,
  mut images: ResMut<Assets<Image>>,
  mut meshes: ResMut<Assets<Mesh>>,
  mut blit_materials: ResMut<Assets<PixelBlitMaterial>>,
  camera_query: Query<
    (Entity, &Transform, &Projection),
    (With<PixelCamera>, Without<PixelSceneCamera>),
  >,
  windows: Query<&Window>,
) {
  // Skip if already initialized
  if state.initialized {
    return;
  }

  // Get the camera with PixelCamera marker
  let Ok((camera_entity, camera_transform, projection)) = camera_query.single() else {
    return;
  };

  // Get window dimensions for aspect ratio
  let Ok(window) = windows.single() else {
    return;
  };

  let window_width = window.physical_width();
  let window_height = window.physical_height();

  if window_width == 0 || window_height == 0 {
    return;
  }

  // Get orthographic projection for pixel size calculation
  let Projection::Orthographic(ortho) = projection else {
    warn!("PixelCamera requires an orthographic projection");
    return;
  };

  // Wait until Bevy has computed the orthographic area (happens after first
  // frame)
  if ortho.area.max.y <= ortho.area.min.y {
    return;
  }

  // Calculate target dimensions based on pixel size mode
  let (target_width, target_height, pixel_world_size) =
    calculate_target_dimensions(&config, ortho, window_width, window_height);

  // Add margin for subpixel offset
  let margin = config.margin;
  let total_width = target_width + margin * 2;
  let total_height = target_height + margin * 2;

  debug!(
    "Pixel camera: {}x{} target ({}px margin), pixel_world_size={}",
    target_width, target_height, margin, pixel_world_size
  );

  // Create render target image
  let size = Extent3d {
    width: total_width,
    height: total_height,
    depth_or_array_layers: 1,
  };

  let mut render_target = Image {
    texture_descriptor: TextureDescriptor {
      label: Some("pixel_camera_target"),
      size,
      dimension: TextureDimension::D2,
      format: TextureFormat::Rgba8UnormSrgb,
      mip_level_count: 1,
      sample_count: 1,
      usage: TextureUsages::TEXTURE_BINDING
        | TextureUsages::COPY_DST
        | TextureUsages::RENDER_ATTACHMENT,
      view_formats: &[],
    },
    // Use nearest-neighbor sampling for pixel-perfect rendering
    sampler: ImageSampler::nearest(),
    ..default()
  };
  render_target.resize(size);

  let render_target_handle = images.add(render_target);

  // Convert the existing camera into the scene camera (renders to low-res target)
  // Scene camera sees layers 0-29 (excludes fullres and blit layers)
  let scene_layers: RenderLayers = (0..FULLRES_SPRITE_LAYER).collect();

  // Fixed orthographic projection that exactly matches render target dimensions
  // This ensures 1 world unit = 1 pixel (no scaling artifacts)
  let half_width = total_width as f32 * pixel_world_size / 2.0;
  let half_height = total_height as f32 * pixel_world_size / 2.0;
  let scene_projection = Projection::Orthographic(OrthographicProjection {
    near: -1000.0,
    far: 1000.0,
    scale: 1.0,
    viewport_origin: Vec2::new(0.5, 0.5),
    scaling_mode: bevy::camera::ScalingMode::Fixed {
      width: half_width * 2.0,
      height: half_height * 2.0,
    },
    area: Rect::default(), // Bevy computes from scaling_mode
  });

  commands.entity(camera_entity).insert((
    PixelSceneCamera,
    LogicalCameraPosition(Vec2::new(
      camera_transform.translation.x,
      camera_transform.translation.y,
    )),
    Camera {
      order: -1, // Render first
      target: RenderTarget::Image(render_target_handle.clone().into()),
      clear_color: ClearColorConfig::Custom(Color::BLACK),
      ..default()
    },
    scene_projection,
    scene_layers,
  ));

  // Calculate viewport rect in UV space
  let viewport_rect = Vec4::new(
    margin as f32 / total_width as f32,
    margin as f32 / total_height as f32,
    target_width as f32 / total_width as f32,
    target_height as f32 / total_height as f32,
  );

  // Create blit material
  let blit_material = blit_materials.add(PixelBlitMaterial {
    texture: render_target_handle.clone(),
    uniforms: PixelBlitUniforms {
      subpixel_offset: Vec2::ZERO,
      viewport_rect,
    },
  });

  // Create full-screen quad mesh for blitting
  // The quad should cover the entire screen in normalized device coordinates
  let quad_mesh = meshes.add(Rectangle::new(2.0, 2.0));

  // Spawn blit camera - projection must be set in the same spawn to prevent
  // Camera2d's required component defaults from overriding it
  commands.spawn((
    Name::new("PixelBlitCamera"),
    PixelBlitCamera,
    Camera2d,
    Camera {
      order: 0, // Render after scene camera
      clear_color: ClearColorConfig::Custom(Color::BLACK),
      ..default()
    },
    Projection::Orthographic(OrthographicProjection {
      near: -1.0,
      far: 1.0,
      scale: 1.0,
      viewport_origin: Vec2::new(0.5, 0.5),
      scaling_mode: bevy::camera::ScalingMode::Fixed {
        width: 2.0,
        height: 2.0,
      },
      area: Rect::default(),
    }),
    RenderLayers::layer(BLIT_LAYER), // Only render blit layer
  ));

  // Spawn blit quad
  commands.spawn((
    Name::new("PixelBlitQuad"),
    PixelBlitQuad,
    Mesh2d(quad_mesh),
    MeshMaterial2d(blit_material),
    Transform::from_xyz(0.0, 0.0, 0.0),
    Visibility::default(),
    RenderLayers::layer(BLIT_LAYER), // Only visible to blit camera
  ));

  // Spawn full-resolution camera - projection must be set in the same spawn
  commands.spawn((
    Name::new("PixelFullresCamera"),
    PixelFullresCamera,
    Camera2d,
    Camera {
      order: 1, // Render after blit
      clear_color: ClearColorConfig::None,
      ..default()
    },
    Projection::Orthographic(ortho.clone()),
    Transform::from_translation(camera_transform.translation),
    RenderLayers::layer(FULLRES_SPRITE_LAYER),
  ));

  // Update state
  state.render_target = render_target_handle;
  state.target_size = UVec2::new(total_width, total_height);
  state.pixel_world_size = pixel_world_size;
  state.initialized = true;
}

/// System: Fixes camera projections once after spawn.
///
/// Camera2d's required components can override explicitly set projections.
/// This system runs once after initialization to force the correct projections
/// on the blit and fullres cameras.
///
/// See: https://github.com/bevyengine/bevy/issues/16556
pub fn fix_camera_projections(
  mut has_run: Local<bool>,
  state: Res<PixelCameraState>,
  mut blit_camera_query: Query<
    &mut Projection,
    (With<PixelBlitCamera>, Without<PixelFullresCamera>),
  >,
  mut fullres_camera_query: Query<(&mut Projection, &PixelFullresCamera), Without<PixelBlitCamera>>,
  scene_camera_query: Query<
    &Projection,
    (
      With<PixelSceneCamera>,
      Without<PixelBlitCamera>,
      Without<PixelFullresCamera>,
    ),
  >,
) {
  if *has_run || !state.initialized {
    return;
  }

  // Fix blit camera projection - must be Fixed 2x2 to fill the screen with the
  // blit quad
  for mut projection in blit_camera_query.iter_mut() {
    *projection = Projection::Orthographic(OrthographicProjection {
      near: -1.0,
      far: 1.0,
      scale: 1.0,
      viewport_origin: Vec2::new(0.5, 0.5),
      scaling_mode: bevy::camera::ScalingMode::Fixed {
        width: 2.0,
        height: 2.0,
      },
      area: Rect::default(),
    });
  }

  // Fix fullres camera projection - must match scene camera
  if let Ok(scene_projection) = scene_camera_query.single() {
    for (mut projection, _) in fullres_camera_query.iter_mut() {
      *projection = scene_projection.clone();
    }
  }

  *has_run = true;
}

/// Calculates target dimensions based on pixel size mode.
fn calculate_target_dimensions(
  config: &PixelCameraConfig,
  ortho: &OrthographicProjection,
  window_width: u32,
  window_height: u32,
) -> (u32, u32, f32) {
  let aspect_ratio = window_width as f32 / window_height as f32;

  // Get the orthographic half-height from area or scale
  let half_height = if ortho.area.max.y > ortho.area.min.y {
    (ortho.area.max.y - ortho.area.min.y) / 2.0
  } else {
    // Area not computed yet, use scale as fallback
    ortho.scale * 100.0 // Rough estimate
  };

  match config.pixel_size_mode {
    PixelSizeMode::FixedVerticalResolution(height) => {
      let target_height = height;
      let target_width = (height as f32 * aspect_ratio).ceil() as u32;
      let pixel_world_size = (2.0 * half_height) / height as f32;
      (target_width, target_height, pixel_world_size)
    }
    PixelSizeMode::WorldSpacePixelSize(size) => {
      let pixel_world_size = size;
      let target_height = ((2.0 * half_height) / size).ceil() as u32;
      let target_width = (target_height as f32 * aspect_ratio).ceil() as u32;
      (target_width, target_height, pixel_world_size)
    }
  }
}
