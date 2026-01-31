/*
   Phosphor Afterglow Shader - WGSL port

   Original: Copyright (C) 2020 guest(r) - guest.r@gmail.com
   License: GPL-3.0-or-later
*/

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

// Parameters
const PR: f32 = 0.12;
const PG: f32 = 0.12;
const PB: f32 = 0.12;

@group(2) @binding(0) var source_texture: texture_2d<f32>;
@group(2) @binding(1) var source_sampler: sampler;
@group(2) @binding(2) var<uniform> texture_size: vec2<f32>;
@group(2) @binding(3) var feedback_texture: texture_2d<f32>;
@group(2) @binding(4) var feedback_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;

    let color = textureSample(source_texture, source_sampler, uv).rgb;
    let accumulate = textureSample(feedback_texture, feedback_sampler, uv).rgb;

    // Skip persistence for very dark pixels
    var w = 1.0;
    if (color.r + color.g + color.b) < (25.0 / 255.0) {
        w = 0.0;
    }

    // Blend current frame with accumulated persistence
    let persistence = vec3<f32>(PR, PG, PB);
    let blended = max(mix(color, accumulate, 0.49 + persistence) - 2.0 / 255.0, vec3<f32>(0.0));
    let result = mix(blended, color, w);

    return vec4<f32>(result, 1.0);
}
