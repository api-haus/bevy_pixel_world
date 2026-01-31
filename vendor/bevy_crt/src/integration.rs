//! Integration with bevy_pixel_world's PixelCamera.
//!
//! When PixelCamera is present, CRT processing is inserted between
//! the pixel blit and the screen output.
//!
//! # Usage
//!
//! Use `setup_crt_with_pixel_camera` as a setup system, passing the
//! `PixelBlitCamera` marker component from bevy_pixel_world.

use bevy::{
  camera::RenderTarget,
  prelude::*,
  render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
  },
};
use bevy_pixel_world::PixelCameraState;

use crate::materials::*;
use crate::plugin::{CrtRenderTargets, CrtState, spawn_crt_pass};

/// Layer offset for CRT passes when integrated with PixelCamera.
const CRT_INTEGRATION_LAYER_BASE: usize = 20;

/// System: Integrates CRT with an existing PixelCamera setup.
///
/// This modifies the PixelCamera blit camera to render to an intermediate
/// target instead of the screen, then applies CRT processing.
///
/// The generic parameter `BlitMarker` should be the `PixelBlitCamera` component
/// from bevy_pixel_world.
#[allow(clippy::too_many_arguments)]
pub fn setup_crt_with_pixel_camera<BlitMarker: Component>(
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
  blit_camera_query: Query<(Entity, &Camera), With<BlitMarker>>,
  windows: Query<&Window>,
  pixel_camera_state: Option<Res<PixelCameraState>>,
) {
  // Skip if already initialized
  if state.initialized {
    return;
  }

  // Get the PixelCamera blit camera
  let Ok((blit_camera_entity, blit_camera)) = blit_camera_query.single() else {
    return; // No PixelCamera present, fall back to standard CRT
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

  info!("CRT: Integrating with PixelCamera at {}x{}", width, height);

  // Create intermediate target for blit output (CRT source)
  let blit_output = create_render_target(size, &mut images);

  // Create all CRT intermediate targets
  let afterglow_out = create_render_target(size, &mut images);
  let afterglow_in = create_render_target(size, &mut images);
  let pre_target = create_render_target(size, &mut images);
  let linearize_target = create_render_target(size, &mut images);
  let pass1_target = create_render_target(size, &mut images);
  let bloom_h_target = create_render_target(size, &mut images);
  let bloom_v_target = create_render_target(size, &mut images);
  let pass2_target = create_render_target(size, &mut images);

  // Store state
  state.source_target = blit_output.clone();
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

  // Modify blit camera to render to our intermediate target instead of screen
  commands.entity(blit_camera_entity).insert(Camera {
    target: RenderTarget::Image(blit_output.clone().into()),
    ..blit_camera.clone()
  });

  // Create fullscreen quad mesh
  let quad = meshes.add(Rectangle::new(2.0, 2.0));
  let texture_size = Vec2::new(width as f32, height as f32);

  // Get source game resolution (low-res pixel dimensions)
  let source_size = pixel_camera_state
    .as_ref()
    .map(|s| Vec2::new(s.target_size.x as f32, s.target_size.y as f32))
    .unwrap_or(texture_size);

  // CRT passes start after the blit camera's order (0)
  // We use positive orders to render after blit

  // Pass 1: Afterglow
  let afterglow_mat = afterglow_materials.add(AfterglowMaterial {
    source_image: blit_output.clone(),
    texture_size,
    feedback: afterglow_in.clone(),
  });
  spawn_crt_pass(
    &mut commands,
    &quad,
    afterglow_mat,
    RenderTarget::Image(afterglow_out.clone().into()),
    CRT_INTEGRATION_LAYER_BASE,
    10,
  );

  // Pass 2: Pre-shader
  let pre_mat = pre_materials.add(PreMaterial {
    source_image: blit_output.clone(),
    texture_size,
    afterglow: afterglow_in.clone(),
  });
  spawn_crt_pass(
    &mut commands,
    &quad,
    pre_mat,
    RenderTarget::Image(pre_target.clone().into()),
    CRT_INTEGRATION_LAYER_BASE + 1,
    11,
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
    CRT_INTEGRATION_LAYER_BASE + 2,
    12,
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
    CRT_INTEGRATION_LAYER_BASE + 3,
    13,
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
    CRT_INTEGRATION_LAYER_BASE + 4,
    14,
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
    CRT_INTEGRATION_LAYER_BASE + 5,
    15,
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
    CRT_INTEGRATION_LAYER_BASE + 6,
    16,
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
    CRT_INTEGRATION_LAYER_BASE + 7,
    17,
  );

  state.initialized = true;
  info!("CRT: PixelCamera integration complete with 8 passes");
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
