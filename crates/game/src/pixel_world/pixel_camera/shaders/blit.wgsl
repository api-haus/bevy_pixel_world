#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0) var texture: texture_2d<f32>;
@group(2) @binding(1) var texture_sampler: sampler;

struct PixelBlitUniforms {
    subpixel_offset: vec2<f32>,
    viewport_rect: vec4<f32>,
}

@group(2) @binding(2) var<uniform> uniforms: PixelBlitUniforms;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    // Map screen UV to viewport UV (accounting for margin)
    // viewport_rect.xy = margin offset in UV space
    // viewport_rect.zw = viewport size in UV space
    let viewport_uv = uniforms.viewport_rect.xy + mesh.uv * uniforms.viewport_rect.zw;

    // Apply subpixel offset for smooth camera movement
    let sample_uv = viewport_uv + uniforms.subpixel_offset;

    // Point sample (nearest neighbor) - sampler is non_filtering
    return textureSample(texture, texture_sampler, sample_uv);
}
