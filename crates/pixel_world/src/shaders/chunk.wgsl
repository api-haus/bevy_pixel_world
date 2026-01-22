// Chunk fragment shader - samples texture for Material2d rendering.

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var chunk_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var chunk_sampler: sampler;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(chunk_texture, chunk_sampler, mesh.uv);
}
