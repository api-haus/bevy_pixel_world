//! CRT post-processing plugin for Bevy 0.17.
//!
//! Provides a multi-pass CRT effect based on crt-guest-advanced-hd shaders.

use bevy::{
  camera::RenderTarget,
  camera::visibility::RenderLayers,
  ecs::system::SystemParam,
  prelude::*,
  render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
  },
  sprite_render::Material2dPlugin,
};
use bevy_egui::{EguiContext, PrimaryEguiContext};
use bevy_pixel_world::pixel_camera::{PixelBlitCamera, PixelFullresCamera};
use bevy_pixel_world::{PixelCameraConfig, PixelCameraState};

use crate::materials::*;

/// Layer for CRT intermediate passes (high layer to avoid conflicts).
const CRT_PASS_LAYER_BASE: usize = 20;

/// Plugin for 2D CRT post-processing effect.
///
/// Creates a multi-pass rendering pipeline that applies:
/// - Phosphor afterglow/persistence
/// - Color temperature and gamma adjustments
/// - Horizontal and vertical filtering
/// - Bloom/glow effects
/// - Scanlines
/// - CRT mask patterns
/// - Screen curvature
///
/// # Usage
///
/// ```ignore
/// use bevy_crt::Crt2dPlugin;
///
/// app.add_plugins(Crt2dPlugin);
/// ```
///
/// The plugin will automatically post-process your 2D renders.
pub struct Crt2dPlugin;

impl Plugin for Crt2dPlugin {
  fn build(&self, app: &mut App) {
    // Skip if rendering is not available
    if !app.is_plugin_added::<bevy::render::RenderPlugin>() {
      return;
    }

    // Embed shaders
    bevy::asset::embedded_asset!(app, "shaders/afterglow.wgsl");
    bevy::asset::embedded_asset!(app, "shaders/preshader.wgsl");
    bevy::asset::embedded_asset!(app, "shaders/linearize.wgsl");
    bevy::asset::embedded_asset!(app, "shaders/pass1.wgsl");
    bevy::asset::embedded_asset!(app, "shaders/bloom_horizontal.wgsl");
    bevy::asset::embedded_asset!(app, "shaders/bloom_vertical.wgsl");
    bevy::asset::embedded_asset!(app, "shaders/pass2.wgsl");
    bevy::asset::embedded_asset!(app, "shaders/deconvergence.wgsl");
    bevy::asset::embedded_asset!(app, "shaders/bypass.wgsl");

    // Register materials
    app.add_plugins((
      Material2dPlugin::<AfterglowMaterial>::default(),
      Material2dPlugin::<PreMaterial>::default(),
      Material2dPlugin::<LinearizeMaterial>::default(),
      Material2dPlugin::<PostMaterial>::default(),
      Material2dPlugin::<BloomHorizontal>::default(),
      Material2dPlugin::<BloomVertical>::default(),
      Material2dPlugin::<PostMaterial2>::default(),
      Material2dPlugin::<DeconvergenceMaterial>::default(),
      Material2dPlugin::<BypassMaterial>::default(),
    ));

    // Initialize resources
    app.init_resource::<CrtState>();
    app.init_resource::<CrtConfig>();
    app.init_resource::<PixelCameraIntegrated>();

    // Setup system
    app.add_systems(Update, setup_crt_pipeline.run_if(not(crt_initialized)));

    // Per-frame systems
    app.add_systems(
      PostUpdate,
      (update_frame_count, update_crt_params).run_if(crt_initialized),
    );

    // Toggle system - runs in Update (before rendering extracts camera data)
    app.add_systems(Update, toggle_crt_pipeline.run_if(crt_initialized));

    // One-shot system to integrate with pixel camera (runs once after CRT init)
    app.add_systems(
      PostUpdate,
      configure_pixel_camera_integration
        .run_if(crt_initialized)
        .run_if(not(pixel_camera_integrated)),
    );
  }
}

/// Track whether pixel camera integration has run.
#[derive(Resource, Default)]
struct PixelCameraIntegrated(bool);

fn pixel_camera_integrated(integrated: Option<Res<PixelCameraIntegrated>>) -> bool {
  integrated.is_some_and(|i| i.0)
}

/// Run condition: CRT pipeline is initialized.
fn crt_initialized(state: Res<CrtState>) -> bool {
  state.initialized
}

/// CRT effect configuration with live-reloadable parameters.
#[derive(Resource, Clone, Debug, serde::Deserialize)]
#[serde(default)]
pub struct CrtConfig {
  /// Master enable/disable toggle.
  pub enabled: bool,
  /// Horizontal curvature (0.0 = flat, 0.03 = subtle, 0.1 = strong).
  pub curvature_x: f32,
  /// Vertical curvature (0.0 = flat, 0.04 = subtle, 0.1 = strong).
  pub curvature_y: f32,
  /// Scanline intensity (0.0 = none, 0.6 = visible, 1.0 = maximum).
  pub scanline_intensity: f32,
  /// Scanline sharpness (0.5 = soft, 0.75 = medium, 1.0 = sharp).
  pub scanline_sharpness: f32,
  /// CRT mask strength (0.0 = none, 0.3 = subtle, 1.0 = strong).
  pub mask_strength: f32,
  /// Mask type: 0 = phosphor, 2 = aperture grille, 6 = trinitron, -1 = none.
  pub mask_type: i32,
  /// Glow/bloom intensity (0.0 = none, 0.08 = subtle, 0.3 = strong).
  pub glow: f32,
  /// Brightness boost (1.0 = normal, 1.4 = brighter).
  pub brightness: f32,
  /// Output gamma (1.0 = linear, 1.75 = typical CRT, 2.2 = sRGB).
  pub gamma: f32,
  /// Corner border size (0.0 = sharp corners, 0.01 = subtle rounding).
  pub corner_size: f32,
  /// Humbar speed in frames per cycle (higher = slower, 50 = default).
  pub humbar_speed: f32,
  /// Humbar intensity (0.0 = disabled, 0.1 = subtle, negative = reverse).
  pub humbar_intensity: f32,
}

impl Default for CrtConfig {
  fn default() -> Self {
    Self {
      enabled: true,
      curvature_x: 0.03,
      curvature_y: 0.04,
      scanline_intensity: 0.6,
      scanline_sharpness: 0.75,
      mask_strength: 0.3,
      mask_type: 0,
      glow: 0.08,
      brightness: 1.4,
      gamma: 1.75,
      corner_size: 0.01,
      humbar_speed: 50.0,
      humbar_intensity: 0.1,
    }
  }
}

impl CrtConfig {
  /// Convert config to shader-compatible CrtParams.
  pub fn to_params(&self) -> CrtParams {
    CrtParams {
      curvature: Vec2::new(self.curvature_x, self.curvature_y),
      scanline: Vec2::new(self.scanline_intensity, self.scanline_sharpness),
      mask: Vec2::new(self.mask_strength, self.mask_type as f32),
      glow_brightness: Vec2::new(self.glow, self.brightness),
      gamma_corner: Vec2::new(self.gamma, self.corner_size),
      humbar: Vec2::new(self.humbar_speed, self.humbar_intensity),
      enabled: UVec4::new(u32::from(self.enabled), 0, 0, 0),
    }
  }
}

/// Runtime state for CRT rendering.
#[derive(Resource, Default)]
pub struct CrtState {
  /// Pipeline is set up.
  pub initialized: bool,
  /// Source render target handle.
  pub source_target: Handle<Image>,
  /// Intermediate render targets.
  pub render_targets: CrtRenderTargets,
  /// Whether CRT pipeline is currently enabled.
  pub enabled: bool,
}

/// Handles to all intermediate render targets.
#[derive(Default, Clone)]
pub struct CrtRenderTargets {
  pub afterglow_out: Handle<Image>,
  pub afterglow_in: Handle<Image>,
  pub pre: Handle<Image>,
  pub linearize: Handle<Image>,
  pub pass1: Handle<Image>,
  pub bloom_h: Handle<Image>,
  pub bloom_v: Handle<Image>,
  pub pass2: Handle<Image>,
}

/// Marker for entities that are part of the CRT pipeline.
#[derive(Component)]
pub struct CrtPipelineEntity;

/// Marker for the bypass camera (renders directly to window when CRT disabled).
#[derive(Component)]
pub struct CrtBypassCamera;

/// Marker for the source camera that should be CRT-processed.
///
/// Add this to your game camera to enable CRT post-processing.
#[derive(Component, Default)]
pub struct CrtSourceCamera;

/// SystemParam bundle for all CRT material asset resources.
#[derive(SystemParam)]
struct CrtMaterials<'w> {
  afterglow: ResMut<'w, Assets<AfterglowMaterial>>,
  pre: ResMut<'w, Assets<PreMaterial>>,
  linearize: ResMut<'w, Assets<LinearizeMaterial>>,
  post: ResMut<'w, Assets<PostMaterial>>,
  bloom_h: ResMut<'w, Assets<BloomHorizontal>>,
  bloom_v: ResMut<'w, Assets<BloomVertical>>,
  post2: ResMut<'w, Assets<PostMaterial2>>,
  decon: ResMut<'w, Assets<DeconvergenceMaterial>>,
  bypass: ResMut<'w, Assets<BypassMaterial>>,
}

/// Sets up the CRT post-processing pipeline.
///
/// Automatically detects PixelBlitCamera from bevy_pixel_world (by name)
/// and integrates with it, or falls back to looking for CrtSourceCamera.
#[allow(clippy::too_many_arguments)]
fn setup_crt_pipeline(
  mut commands: Commands,
  mut state: ResMut<CrtState>,
  crt_config: Res<CrtConfig>,
  mut images: ResMut<Assets<Image>>,
  mut meshes: ResMut<Assets<Mesh>>,
  mut mats: CrtMaterials,
  source_camera_query: Query<(Entity, &Camera), With<CrtSourceCamera>>,
  pixel_blit_camera_query: Query<(Entity, &Camera, &Name)>,
  windows: Query<&Window>,
  pixel_camera_state: Option<Res<PixelCameraState>>,
) {
  // Skip if already initialized
  if state.initialized {
    return;
  }

  // First, try to find PixelBlitCamera by name (from bevy_pixel_world)
  let source_camera = pixel_blit_camera_query
    .iter()
    .find(|(_, _, name)| name.as_str() == "PixelBlitCamera")
    .map(|(e, c, _)| (e, c, true));

  // Fall back to CrtSourceCamera marker
  let (camera_entity, camera, is_pixel_camera) = match source_camera {
    Some((e, c, _)) => (e, c, true),
    None => {
      match source_camera_query.single() {
        Ok((e, c)) => (e, c, false),
        Err(_) => return, // No camera found yet
      }
    }
  };

  // Get window dimensions
  let Ok(window) = windows.single() else {
    return;
  };

  let width = window.physical_width();
  let height = window.physical_height();

  if width == 0 || height == 0 {
    return;
  }

  let size = Extent3d {
    width,
    height,
    depth_or_array_layers: 1,
  };

  if is_pixel_camera {
    info!("CRT: Integrating with PixelCamera at {}x{}", width, height);
  } else {
    info!(
      "CRT: Setting up standalone pipeline at {}x{}",
      width, height
    );
  }

  // Create render targets
  let source_target = create_render_target(size, &mut images);
  let afterglow_out = create_render_target(size, &mut images);
  let afterglow_in = create_render_target(size, &mut images);
  let pre_target = create_render_target(size, &mut images);
  let linearize_target = create_render_target(size, &mut images);
  let pass1_target = create_render_target(size, &mut images);
  let bloom_h_target = create_render_target(size, &mut images);
  let bloom_v_target = create_render_target(size, &mut images);
  let pass2_target = create_render_target(size, &mut images);

  // Store state
  state.source_target = source_target.clone();
  state.render_targets = CrtRenderTargets {
    afterglow_out: afterglow_out.clone(),
    afterglow_in: afterglow_in.clone(),
    pre: pre_target.clone(),
    linearize: linearize_target.clone(),
    pass1: pass1_target.clone(),
    bloom_h: bloom_h_target.clone(),
    bloom_v: bloom_v_target.clone(),
    pass2: pass2_target.clone(),
  };

  // Update source camera to render to our intermediate target
  // For PixelBlitCamera: preserve its order but redirect output
  // For CrtSourceCamera: set order to -10 to render first
  let source_order = if is_pixel_camera { camera.order } else { -10 };
  state.enabled = true;

  commands.entity(camera_entity).insert(Camera {
    target: RenderTarget::Image(source_target.clone().into()),
    order: source_order,
    ..camera.clone()
  });

  // CRT passes need orders higher than the source camera
  let base_order: isize = if is_pixel_camera { 10 } else { 1 };

  // Create fullscreen quad mesh
  let quad = meshes.add(Rectangle::new(2.0, 2.0));
  // Pad to Vec4 for WebGL 16-byte alignment
  let texture_size = Vec4::new(width as f32, height as f32, 0.0, 0.0);

  // Get source game resolution (low-res pixel dimensions)
  let source_size = pixel_camera_state
    .as_ref()
    .map(|s| Vec4::new(s.target_size.x as f32, s.target_size.y as f32, 0.0, 0.0))
    .unwrap_or(texture_size);

  // Pass 1: Afterglow
  let afterglow_mat = mats.afterglow.add(AfterglowMaterial {
    source_image: source_target.clone(),
    texture_size,
    feedback: afterglow_in.clone(),
  });
  spawn_crt_pass(
    &mut commands,
    &quad,
    afterglow_mat,
    RenderTarget::Image(afterglow_out.clone().into()),
    CRT_PASS_LAYER_BASE,
    base_order,
  );

  // Pass 2: Pre-shader
  let pre_mat = mats.pre.add(PreMaterial {
    source_image: source_target.clone(),
    texture_size,
    afterglow: afterglow_in.clone(),
  });
  spawn_crt_pass(
    &mut commands,
    &quad,
    pre_mat,
    RenderTarget::Image(pre_target.clone().into()),
    CRT_PASS_LAYER_BASE + 1,
    base_order + 1,
  );

  // Pass 3: Linearize
  let linearize_mat = mats.linearize.add(LinearizeMaterial {
    source_image: pre_target.clone(),
    texture_size,
    frame_count: UVec4::ZERO,
  });
  spawn_crt_pass(
    &mut commands,
    &quad,
    linearize_mat,
    RenderTarget::Image(linearize_target.clone().into()),
    CRT_PASS_LAYER_BASE + 2,
    base_order + 2,
  );

  // Pass 4: Pass1 (horizontal filtering)
  let pass1_mat = mats.post.add(PostMaterial {
    linearize_pass: linearize_target.clone(),
    texture_size,
  });
  spawn_crt_pass(
    &mut commands,
    &quad,
    pass1_mat,
    RenderTarget::Image(pass1_target.clone().into()),
    CRT_PASS_LAYER_BASE + 3,
    base_order + 3,
  );

  // Pass 5: Bloom horizontal
  let bloom_h_mat = mats.bloom_h.add(BloomHorizontal {
    linearize_pass: linearize_target.clone(),
    texture_size,
  });
  spawn_crt_pass(
    &mut commands,
    &quad,
    bloom_h_mat,
    RenderTarget::Image(bloom_h_target.clone().into()),
    CRT_PASS_LAYER_BASE + 4,
    base_order + 4,
  );

  // Pass 6: Bloom vertical
  let bloom_v_mat = mats.bloom_v.add(BloomVertical {
    source_image: bloom_h_target.clone(),
    texture_size,
  });
  spawn_crt_pass(
    &mut commands,
    &quad,
    bloom_v_mat,
    RenderTarget::Image(bloom_v_target.clone().into()),
    CRT_PASS_LAYER_BASE + 5,
    base_order + 5,
  );

  // Pass 7: Pass2 (vertical filtering + scanlines)
  let pass2_mat = mats.post2.add(PostMaterial2 {
    pass_1: pass1_target.clone(),
    texture_size,
    linearize_pass: linearize_target.clone(),
    source_size,
  });
  spawn_crt_pass(
    &mut commands,
    &quad,
    pass2_mat,
    RenderTarget::Image(pass2_target.clone().into()),
    CRT_PASS_LAYER_BASE + 6,
    base_order + 6,
  );

  // Pass 8: Deconvergence (final - renders to screen)
  let decon_mat = mats.decon.add(DeconvergenceMaterial {
    source_image: pass2_target.clone(),
    texture_size,
    linearize_pass: linearize_target.clone(),
    bloom_pass: bloom_v_target.clone(),
    pre_pass: pre_target.clone(),
    frame_count: UVec4::ZERO,
    source_size,
    params: crt_config.to_params(),
  });
  spawn_crt_pass(
    &mut commands,
    &quad,
    decon_mat,
    RenderTarget::Window(bevy::window::WindowRef::Primary),
    CRT_PASS_LAYER_BASE + 7,
    base_order + 7,
  );

  // Bypass camera: renders directly to window when CRT is disabled
  // Uses same order as deconvergence but is initially disabled
  let bypass_mat = mats.bypass.add(BypassMaterial {
    source_image: source_target.clone(),
  });
  let bypass_layer = CRT_PASS_LAYER_BASE + 8;
  let bypass_render_layer = RenderLayers::layer(bypass_layer);

  // Spawn bypass quad
  commands.spawn((
    Name::new("CrtBypassQuad"),
    Mesh2d(quad.clone()),
    MeshMaterial2d(bypass_mat),
    Transform::from_xyz(0.0, 0.0, 0.0),
    Visibility::default(),
    bypass_render_layer.clone(),
  ));

  // Spawn bypass camera (initially disabled)
  commands.spawn((
    Name::new("CrtBypassCamera"),
    CrtBypassCamera,
    Camera2d,
    Camera {
      order: base_order + 7, // Same order as deconvergence
      target: RenderTarget::Window(bevy::window::WindowRef::Primary),
      clear_color: ClearColorConfig::None,
      is_active: false, // Disabled by default
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
    bypass_render_layer,
  ));

  state.initialized = true;
  info!("CRT: Pipeline initialized with 8 passes + bypass");
}

/// Spawns a CRT pass (quad + camera).
pub fn spawn_crt_pass<M: bevy::sprite_render::Material2d>(
  commands: &mut Commands,
  quad: &Handle<Mesh>,
  material: Handle<M>,
  target: RenderTarget,
  layer: usize,
  order: isize,
) {
  let render_layer = RenderLayers::layer(layer);

  // Spawn quad
  commands.spawn((
    Name::new(format!("CrtQuad_{}", order)),
    CrtPipelineEntity,
    Mesh2d(quad.clone()),
    MeshMaterial2d(material),
    Transform::from_xyz(0.0, 0.0, 0.0),
    Visibility::default(),
    render_layer.clone(),
  ));

  // Spawn camera
  commands.spawn((
    Name::new(format!("CrtCamera_{}", order)),
    CrtPipelineEntity,
    Camera2d,
    Camera {
      order,
      target,
      clear_color: ClearColorConfig::None,
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
    render_layer,
  ));
}

/// Creates a render target image.
fn create_render_target(size: Extent3d, images: &mut ResMut<Assets<Image>>) -> Handle<Image> {
  let mut image = Image {
    texture_descriptor: TextureDescriptor {
      label: Some("crt_render_target"),
      size,
      dimension: TextureDimension::D2,
      format: TextureFormat::Rgba8UnormSrgb,
      mip_level_count: 1,
      sample_count: 1,
      usage: TextureUsages::TEXTURE_BINDING
        | TextureUsages::COPY_DST
        | TextureUsages::COPY_SRC
        | TextureUsages::RENDER_ATTACHMENT,
      view_formats: &[],
    },
    ..default()
  };
  image.resize(size);
  images.add(image)
}

/// Updates frame count in materials that need it (for interlacing, noise,
/// etc.). Skips when CRT is disabled to avoid unnecessary GPU uploads.
fn update_frame_count(
  crt_config: Res<CrtConfig>,
  time: Res<Time>,
  mut linearize_materials: ResMut<Assets<LinearizeMaterial>>,
  mut decon_materials: ResMut<Assets<DeconvergenceMaterial>>,
) {
  if !crt_config.enabled {
    return;
  }

  let frame = (time.elapsed_secs() * 60.0) as u32; // Approximate frame count
  let frame_uvec4 = UVec4::new(frame, 0, 0, 0);

  for (_, material) in linearize_materials.iter_mut() {
    material.frame_count = frame_uvec4;
  }

  for (_, material) in decon_materials.iter_mut() {
    material.frame_count = frame_uvec4;
  }
}

/// Updates CRT parameters in deconvergence material when config changes.
///
/// Uses the same iter_mut() pattern as update_frame_count which already
/// successfully updates materials every frame.
fn update_crt_params(
  crt_config: Res<CrtConfig>,
  mut decon_materials: ResMut<Assets<DeconvergenceMaterial>>,
) {
  if crt_config.is_changed() {
    let params = crt_config.to_params();
    info!(
      "CRT: Updating params - enabled={}, brightness={}, curvature=({}, {})",
      params.enabled, params.glow_brightness.y, params.curvature.x, params.curvature.y
    );

    for (_, material) in decon_materials.iter_mut() {
      material.params = params;
    }
  }
}

/// Toggles CRT pipeline on/off based on CrtConfig.enabled.
///
/// When disabled:
/// - Deactivates all CRT pass cameras
/// - Activates the bypass camera (renders source directly to window)
///
/// When enabled:
/// - Activates all CRT pass cameras
/// - Deactivates the bypass camera
fn toggle_crt_pipeline(
  crt_config: Res<CrtConfig>,
  mut crt_state: ResMut<CrtState>,
  mut crt_cameras: Query<
    &mut Camera,
    (
      With<CrtPipelineEntity>,
      Without<CrtBypassCamera>,
      Without<PixelBlitCamera>,
    ),
  >,
  mut bypass_camera: Query<
    &mut Camera,
    (
      With<CrtBypassCamera>,
      Without<CrtPipelineEntity>,
      Without<PixelBlitCamera>,
    ),
  >,
) {
  if !crt_state.initialized || crt_state.enabled == crt_config.enabled {
    return;
  }
  crt_state.enabled = crt_config.enabled;

  if crt_config.enabled {
    // Re-enable: activate CRT cameras, deactivate bypass
    for mut cam in &mut crt_cameras {
      cam.is_active = true;
    }
    if let Ok(mut cam) = bypass_camera.single_mut() {
      cam.is_active = false;
    }
    info!("CRT: Pipeline enabled");
  } else {
    // Disable: deactivate CRT cameras, activate bypass
    for mut cam in &mut crt_cameras {
      cam.is_active = false;
    }
    if let Ok(mut cam) = bypass_camera.single_mut() {
      cam.is_active = true;
    }
    info!("CRT: Pipeline disabled (bypass mode)");
  }
}

/// System: Integrates CRT with pixel camera for proper egui rendering.
///
/// - Disables pixel camera's egui handling to prevent conflicts
/// - Updates PixelFullresCamera order to render after CRT final pass
/// - Moves egui context from PixelBlitCamera to PixelFullresCamera
///
/// This ensures egui renders on top of CRT effects at full resolution.
fn configure_pixel_camera_integration(
  mut commands: Commands,
  mut integrated: ResMut<PixelCameraIntegrated>,
  mut pixel_config: ResMut<PixelCameraConfig>,
  mut fullres_camera: Query<&mut Camera, (With<PixelFullresCamera>, Without<PixelBlitCamera>)>,
  blit_camera: Query<Entity, (With<PixelBlitCamera>, With<EguiContext>)>,
  fullres_entity: Query<Entity, (With<PixelFullresCamera>, Without<EguiContext>)>,
) {
  // Disable pixel camera's egui handling - CRT will manage it
  pixel_config.egui_full_resolution = false;

  // Update PixelFullresCamera order to be after CRT deconvergence pass (order 17)
  for mut camera in fullres_camera.iter_mut() {
    camera.order = 20; // After CRT final pass (base_order 10 + 7 = 17)
  }

  // Move egui from PixelBlitCamera to PixelFullresCamera
  for entity in blit_camera.iter() {
    commands
      .entity(entity)
      .remove::<EguiContext>()
      .remove::<PrimaryEguiContext>();
  }
  for entity in fullres_entity.iter() {
    commands
      .entity(entity)
      .insert((EguiContext::default(), PrimaryEguiContext));
  }

  integrated.0 = true;
  info!("CRT: Integrated with pixel camera - egui on PixelFullresCamera (order 20)");
}
