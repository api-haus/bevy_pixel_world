//! GPU texture creation and upload for surfaces.
//!
//! Provides utilities to create Bevy textures from surfaces and upload pixel data.

use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::image::ImageSampler;

use crate::surface::RgbaSurface;

/// Creates an RGBA8 texture with nearest-neighbor sampling.
///
/// Returns a handle to the created image.
pub fn create_texture(images: &mut Assets<Image>, width: u32, height: u32) -> Handle<Image> {
    let size = Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    let mut image = Image::new_fill(
        size,
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );

    // Use nearest-neighbor sampling for pixel-perfect rendering
    image.sampler = ImageSampler::nearest();

    images.add(image)
}

/// Uploads surface pixel data to an existing texture.
///
/// The surface dimensions must match the texture dimensions.
pub fn upload_surface(surface: &RgbaSurface, image: &mut Image) {
    let bytes = surface.as_bytes();
    if let Some(ref mut data) = image.data {
        data.copy_from_slice(bytes);
    }
}
