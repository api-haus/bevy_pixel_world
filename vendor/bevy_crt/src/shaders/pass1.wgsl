/*
   CRT Pass1 - Horizontal filtering - WGSL port

   Original: Copyright (C) 2018-2022 guest(r) - guest.r@gmail.com
   License: GPL-3.0-or-later
*/

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

// Parameters
const INTERNAL_RES: f32 = 1.0;
const H_SHARPNESS: f32 = 1.0;
const SIGMA_HOR: f32 = 0.50;
const S_SHARP: f32 = 1.0;
const H_SHARP: f32 = 1.25;
const H_ARNG: f32 = 0.2;
const PRESCALE_X: f32 = 1.0;
const PRESCALE_Y: f32 = 1.0;
const SPIKE: f32 = 1.0;

@group(2) @binding(0) var source_texture: texture_2d<f32>;
@group(2) @binding(1) var source_sampler: sampler;
@group(2) @binding(2) var<uniform> texture_size: vec2<f32>;

fn gaussian(x: f32) -> f32 {
    let inv_sigma_sq = 1.0 / (2.0 * SIGMA_HOR * SIGMA_HOR * INTERNAL_RES * INTERNAL_RES);
    return exp(-x * x * inv_sigma_sq);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;

    let inv_text_size = 1.0 / texture_size;
    let original_size = vec4<f32>(texture_size.x, texture_size.y, inv_text_size.x, inv_text_size.y);
    let source_size = original_size * vec4<f32>(PRESCALE_X, PRESCALE_Y, 1.0 / PRESCALE_X, 1.0 / PRESCALE_Y);

    let f = fract(source_size.x * uv.x) - 0.5;
    let tex = floor(source_size.xy * uv) * source_size.zw + 0.5 * source_size.zw;
    let dx = vec2<f32>(source_size.z, 0.0);

    var color = vec3<f32>(0.0);
    var scolor = 0.0;
    var wsum = 0.0;
    var swsum = 0.0;

    let h_sharpness = H_SHARPNESS * INTERNAL_RES;
    var cmax = vec3<f32>(0.0);
    var cmin = vec3<f32>(1.0);
    let sharp = gaussian(h_sharpness) * S_SHARP;
    let maxsharp = 0.20;
    let fpr = h_sharpness;

    let ts = 0.025;
    let luma = vec3<f32>(0.2126, 0.7152, 0.0722);

    let loop_size = ceil(2.0 * fpr);
    let clamp_size = round(2.0 * loop_size / 3.0);

    var n = -loop_size;
    loop {
        if n > loop_size { break; }

        let pixel = textureSample(source_texture, source_sampler, tex + n * dx).rgb;
        let sp = max(max(pixel.r, pixel.g), pixel.b);

        var w = gaussian(n + f) - sharp;
        let fpx = abs(n + f - sign(n) * fpr) / fpr;

        if abs(n) <= clamp_size {
            cmax = max(cmax, pixel);
            cmin = min(cmin, pixel);
        }

        if w < 0.0 {
            w = clamp(w, mix(-maxsharp, 0.0, pow(fpx, H_SHARP)), 0.0);
        }

        color = color + w * pixel;
        wsum = wsum + w;

        let sw = max(w, 0.0) * (dot(pixel, luma) + ts);
        scolor = scolor + sw * sp;
        swsum = swsum + sw;

        n = n + 1.0;
    }

    color = color / wsum;
    scolor = scolor / swsum;

    color = clamp(mix(clamp(color, cmin, cmax), color, H_ARNG), vec3<f32>(0.0), vec3<f32>(1.0));
    scolor = clamp(mix(max(max(color.r, color.g), color.b), scolor, SPIKE), 0.0, 1.0);

    return vec4<f32>(color, scolor);
}
