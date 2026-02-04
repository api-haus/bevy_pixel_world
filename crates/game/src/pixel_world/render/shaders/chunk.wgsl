// Chunk fragment shader - GPU-side palette lookup for pixel rendering.

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var pixel_texture: texture_2d<u32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var palette_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var palette_sampler: sampler;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    // Load raw pixel data (material, color, damage, flags)
    let dims = textureDimensions(pixel_texture);
    // Clamp UV to [0, 1) to avoid out-of-bounds at chunk edges where UV=1.0
    // Without this, UV=1.0 * 512 = 512, which is out of bounds for 512-pixel texture
    let clamped_uv = clamp(mesh.uv, vec2<f32>(0.0), vec2<f32>(1.0) - 1.0 / vec2<f32>(dims));
    let coord = vec2<i32>(clamped_uv * vec2<f32>(dims));
    let pixel = textureLoad(pixel_texture, coord, 0);

    // Palette layout: material_id * 8 + (color_index * 7 / 255)
    // Maps color_index 0-255 to palette entry 0-7 within the material's color range
    let material_id = pixel.r;
    let color_index = pixel.g;
    let palette_idx = material_id * 8u + (color_index * 7u / 255u);
    let palette_uv = vec2<f32>(f32(palette_idx) + 0.5, 0.5) / vec2<f32>(256.0, 1.0);

    let color = textureSample(palette_texture, palette_sampler, palette_uv);

    return color;
}
