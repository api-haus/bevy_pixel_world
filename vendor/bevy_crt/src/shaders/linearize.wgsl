/*
   Linearize/Interlacing shader - WGSL port

   Original: Copyright (C) 2020-2021 guest(r) - guest.r@gmail.com
   License: GPL-3.0-or-later
*/

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

// Parameters
const GAMMA_INPUT: f32 = 1.80;
const INTER: f32 = 400.0;        // Interlace trigger resolution
const INTERM: f32 = 4.0;         // Interlace mode
const ISCAN: f32 = 0.20;         // Interlacing scanline effect
const INTRES: f32 = 2.0;         // Internal resolution
const PRESCALE_X: f32 = 1.0;
const PRESCALE_Y: f32 = 1.0;
const ISCANS: f32 = 0.25;        // Interlacing saturation

@group(2) @binding(0) var source_texture: texture_2d<f32>;
@group(2) @binding(1) var source_sampler: sampler;
@group(2) @binding(2) var<uniform> texture_size: vec4<f32>;  // Padded for WebGL (xy used)
@group(2) @binding(3) var<uniform> frame_count: vec4<u32>;  // Padded for WebGL 16-byte alignment

fn plant(tar: vec3<f32>, r: f32) -> vec3<f32> {
    let t = max(max(tar.r, tar.g), tar.b) + 0.00001;
    return tar * r / t;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;

    let original_size = vec4<f32>(
        texture_size.x,
        texture_size.y,
        1.0 / texture_size.x,
        1.0 / texture_size.y
    );
    let source_size = original_size * vec4<f32>(PRESCALE_X, PRESCALE_Y, 1.0 / PRESCALE_X, 1.0 / PRESCALE_Y);

    let c1 = textureSample(source_texture, source_sampler, uv).rgb;
    let c2 = textureSample(source_texture, source_sampler, uv + vec2<f32>(0.0, source_size.w)).rgb;

    var c = c1;
    var intera = 1.0;
    let gamma_in = clamp(GAMMA_INPUT, 1.0, 5.0);

    let m1 = max(max(c1.r, c1.g), c1.b);
    let m2 = max(max(c2.r, c2.g), c2.b);
    let df = abs(c1 - c2);
    var d = max(max(df.r, df.g), df.b);

    if INTERM == 2.0 {
        d = mix(0.1 * d, 10.0 * d, step(m1 / (m2 + 0.0001), m2 / (m1 + 0.0001)));
    }

    let r = m1;
    var yres_div = 1.0;
    if INTRES > 1.25 {
        yres_div = INTRES;
    }

    if INTER <= original_size.y / yres_div && INTERM > 0.5 && INTRES != 1.0 && INTRES != 0.5 {
        intera = 0.25;
        let line_no = clamp(floor(original_size.y * uv.y % 2.0), 0.0, 1.0);
        let frame_no = clamp(floor(f32(frame_count.x) % 2.0), 0.0, 1.0);
        let ii = abs(line_no - frame_no);

        if INTERM < 3.5 {
            let c2_plant = plant(mix(c2, c2 * c2, ISCANS), max(max(c2.r, c2.g), c2.b));
            let r_val = clamp(max(m1 * ii, (1.0 - ISCAN) * min(m1, m2)), 0.0, 1.0);
            c = plant(mix(mix(c1, c2_plant, min(mix(m1, 1.0 - m2, min(m1, 1.0 - m1)) / (d + 0.00001), 1.0)), c1, ii), r_val);
            if INTERM == 3.0 {
                c = (1.0 - 0.5 * ISCAN) * mix(c2_plant, c1, ii);
            }
        }
        if INTERM == 4.0 {
            c = plant(mix(c, c * c, 0.5 * ISCANS), max(max(c.r, c.g), c.b));
            intera = 0.45;
        }
    }

    c = pow(c, vec3<f32>(gamma_in));

    // Store gamma info in alpha channel for later passes
    var gamma_out = gamma_in;
    if uv.x > 0.5 {
        gamma_out = intera;
    }

    return vec4<f32>(c, gamma_out);
}
