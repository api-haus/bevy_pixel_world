/*
   Gaussian blur - Vertical pass - WGSL port

   Original: Copyright (C) 2020 guest(r) - guest.r@gmail.com
   License: GPL-3.0-or-later
*/

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

// Parameters
const SIZE_VB: f32 = 4.0;    // Bloom radius
const SIGMA_VB: f32 = 1.0;   // Bloom sigma
const PRESCALE_X: f32 = 1.0;
const PRESCALE_Y: f32 = 1.0;

@group(2) @binding(0) var source_texture: texture_2d<f32>;
@group(2) @binding(1) var source_sampler: sampler;
@group(2) @binding(2) var<uniform> texture_size: vec4<f32>;  // Padded for WebGL (xy used)

fn gaussian(x: f32) -> f32 {
    let inv_sigma_sq = 1.0 / (2.0 * SIGMA_VB * SIGMA_VB);
    return exp(-x * x * inv_sigma_sq);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;

    let inv_text_size = 1.0 / texture_size.xy;
    let original_size = vec4<f32>(texture_size.x, texture_size.y, inv_text_size.x, inv_text_size.y);
    let source_size = original_size * vec4<f32>(PRESCALE_X, PRESCALE_Y, 1.0 / PRESCALE_X, 1.0 / PRESCALE_Y);
    let source_size1 = vec4<f32>(source_size.x, original_size.y, source_size.z, original_size.w);

    let f = fract(source_size1.y * uv.y) - 0.5;
    let tex = floor(source_size1.xy * uv) * source_size1.zw + 0.5 * source_size1.zw;
    let dy = vec2<f32>(0.0, source_size1.w);

    var color = vec4<f32>(0.0);
    var wsum = 0.0;

    var n = -SIZE_VB;
    loop {
        if n > SIZE_VB { break; }

        var pixel = textureSample(source_texture, source_sampler, tex + n * dy);
        let w = gaussian(n + f);

        // Cube the alpha for perceptual weighting
        pixel.a = pixel.a * pixel.a * pixel.a;

        color = color + w * pixel;
        wsum = wsum + w;

        n = n + 1.0;
    }

    color = color / wsum;

    // Length adjustment
    let len = length(color.rgb);
    if len > 0.0 {
        let lenadj = pow(len / sqrt(3.0), 0.75) * sqrt(3.0);
        color = vec4<f32>(color.rgb * (lenadj / len), 1.0);
    }

    return vec4<f32>(color.rgb, 1.0);
}
