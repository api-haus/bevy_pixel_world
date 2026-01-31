/*
   Gaussian blur - Horizontal pass - WGSL port

   Original: Copyright (C) 2021 guest(r) - guest.r@gmail.com
   License: GPL-3.0-or-later
*/

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

// Parameters
const SIZE_HB: f32 = 4.0;    // Bloom radius
const SIGMA_HB: f32 = 1.0;   // Bloom sigma

@group(2) @binding(0) var source_texture: texture_2d<f32>;
@group(2) @binding(1) var source_sampler: sampler;
@group(2) @binding(2) var<uniform> texture_size: vec4<f32>;  // Padded for WebGL (xy used)

fn gaussian(x: f32) -> f32 {
    let inv_sigma_sq = 1.0 / (2.0 * SIGMA_HB * SIGMA_HB);
    return exp(-x * x * inv_sigma_sq);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;

    let inv_text_size = 1.0 / texture_size.xy;
    let source_size = vec4<f32>(texture_size.x, texture_size.y, inv_text_size.x, inv_text_size.y);

    let f = fract(source_size.x * uv.x) - 0.5;
    let tex = floor(source_size.xy * uv) * source_size.zw + 0.5 * source_size.zw;
    let dx = vec2<f32>(source_size.z, 0.0);

    var color = vec4<f32>(0.0);
    var wsum = 0.0;

    var n = -SIZE_HB;
    loop {
        if n > SIZE_HB { break; }

        var pixel = textureSample(source_texture, source_sampler, tex + n * dx);
        let w = gaussian(n + f);

        // Compute luminance for alpha channel
        pixel.a = max(max(pixel.r, pixel.g), pixel.b);
        pixel.a = pixel.a * pixel.a * pixel.a;

        color = color + w * pixel;
        wsum = wsum + w;

        n = n + 1.0;
    }

    color = color / wsum;

    // Length adjustment for perceptual consistency
    let len = length(color.rgb);
    if len > 0.0 {
        let lenadj = pow(len / sqrt(3.0), 0.333333) * sqrt(3.0);
        color = vec4<f32>(color.rgb * (lenadj / len), 1.0);
    }

    return vec4<f32>(color.rgb, 1.0);
}
