//! CRT post-processing plugin for Bevy 0.17.
//!
//! Provides a multi-pass CRT effect based on crt-guest-advanced-hd shaders.

use bevy::{
  camera::RenderTarget,
  camera::visibility::RenderLayers,
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
    ));

    // Initialize resources
    app.init_resource::<CrtState>();
    app.init_resource::<CrtConfig>();
    app.init_resource::<PixelCameraIntegrated>();

    // Setup system
    app.add_systems(Update, setup_crt_pipeline.run_if(not(crt_initialized)));

    // Per-frame systems
    app.add_systems(PostUpdate, update_frame_count.run_if(crt_initialized));

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

/// CRT effect configuration.
#[derive(Resource)]
pub struct CrtConfig {
  /// Enable curvature effect.
  pub curvature: bool,
  /// Enable scanlines.
  pub scanlines: bool,
  /// Enable CRT mask.
  pub mask: bool,
  /// Enable bloom/glow.
  pub bloom: bool,
}

impl Default for CrtConfig {
  fn default() -> Self {
    Self {
      curvature: true,
      scanlines: true,
      mask: true,
      bloom: true,
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

/// Marker for the source camera that should be CRT-processed.
///
/// Add this to your game camera to enable CRT post-processing.
#[derive(Component, Default)]
pub struct CrtSourceCamera;

/// Sets up the CRT post-processing pipeline.
///
/// Automatically detects PixelBlitCamera from bevy_pixel_world (by name)
/// and integrates with it, or falls back to looking for CrtSourceCamera.
#[allow(clippy::too_many_arguments)]
fn setup_crt_pipeline(
  mut commands: Commands,
  mut state: ResMut<CrtState>,
  mut images: ResMut<Assets<Image>>,
  mut meshes: ResMut<Assets<Mesh>>,
  mut afterglow_materials: ResMut<Assets<AfterglowMaterial>>,
  mut pre_materials: ResMut<Assets<PreMaterial>>,
  mut linearize_materials: ResMut<Assets<LinearizeMaterial>>,
  mut post_materials: ResMut<Assets<PostMaterial>>,
  mut bloom_h_materials: ResMut<Assets<BloomHorizontal>>,
  mut bloom_v_materials: ResMut<Assets<BloomVertical>>,
  mut post2_materials: ResMut<Assets<PostMaterial2>>,
  mut decon_materials: ResMut<Assets<DeconvergenceMaterial>>,
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
  commands.entity(camera_entity).insert(Camera {
    target: RenderTarget::Image(source_target.clone().into()),
    order: source_order,
    ..camera.clone()
  });

  // CRT passes need orders higher than the source camera
  let base_order: isize = if is_pixel_camera { 10 } else { 1 };

  // Create fullscreen quad mesh
  let quad = meshes.add(Rectangle::new(2.0, 2.0));
  let texture_size = Vec2::new(width as f32, height as f32);

  // Get source game resolution (low-res pixel dimensions)
  let source_size = pixel_camera_state
    .as_ref()
    .map(|s| Vec2::new(s.target_size.x as f32, s.target_size.y as f32))
    .unwrap_or(texture_size);

  // Pass 1: Afterglow
  let afterglow_mat = afterglow_materials.add(AfterglowMaterial {
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
  let pre_mat = pre_materials.add(PreMaterial {
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
  let linearize_mat = linearize_materials.add(LinearizeMaterial {
    source_image: pre_target.clone(),
    texture_size,
    frame_count: 0,
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
  let pass1_mat = post_materials.add(PostMaterial {
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
  let bloom_h_mat = bloom_h_materials.add(BloomHorizontal {
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
  let bloom_v_mat = bloom_v_materials.add(BloomVertical {
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
  let pass2_mat = post2_materials.add(PostMaterial2 {
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
  let decon_mat = decon_materials.add(DeconvergenceMaterial {
    source_image: pass2_target.clone(),
    texture_size,
    linearize_pass: linearize_target.clone(),
    bloom_pass: bloom_v_target.clone(),
    pre_pass: pre_target.clone(),
    frame_count: 0,
    source_size,
  });
  spawn_crt_pass(
    &mut commands,
    &quad,
    decon_mat,
    RenderTarget::Window(bevy::window::WindowRef::Primary),
    CRT_PASS_LAYER_BASE + 7,
    base_order + 7,
  );

  state.initialized = true;
  info!("CRT: Pipeline initialized with 8 passes");
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
/// etc.).
fn update_frame_count(
  time: Res<Time>,
  mut linearize_materials: ResMut<Assets<LinearizeMaterial>>,
  mut decon_materials: ResMut<Assets<DeconvergenceMaterial>>,
) {
  let frame = (time.elapsed_secs() * 60.0) as u32; // Approximate frame count

  for (_, material) in linearize_materials.iter_mut() {
    material.frame_count = frame;
  }

  for (_, material) in decon_materials.iter_mut() {
    material.frame_count = frame;
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
