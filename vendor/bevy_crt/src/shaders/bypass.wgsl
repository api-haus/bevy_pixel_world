/*
   CRT Bypass Shader - Simple pass-through

   Just samples the input texture and outputs it directly.
   Used when CRT effects are disabled for performance.
*/

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0) var source_texture: texture_2d<f32>;
@group(2) @binding(1) var source_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(source_texture, source_sampler, in.uv);
}
