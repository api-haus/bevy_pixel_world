/*
   CRT Pre-shader (color adjustments + afterglow blend) - WGSL port

   Original: Copyright (C) 2019-2022 guest(r) and Dr. Venom
   License: GPL-3.0-or-later
*/

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

// Parameters
const AS: f32 = 0.20;        // Afterglow strength
const SAT: f32 = 0.5;        // Afterglow saturation
const WP: f32 = 0.0;         // Color temperature
const WP_SATURATION: f32 = 1.0;
const PRE_BB: f32 = 1.0;     // Pre-brightness
const CONTR: f32 = 0.0;      // Contrast
const BP: f32 = 0.0;         // Black point
const CP: f32 = 0.0;         // Color profile
const CS: f32 = 0.0;         // Color space

@group(2) @binding(0) var source_texture: texture_2d<f32>;
@group(2) @binding(1) var source_sampler: sampler;
@group(2) @binding(2) var<uniform> texture_size: vec2<f32>;
@group(2) @binding(3) var afterglow_texture: texture_2d<f32>;
@group(2) @binding(4) var afterglow_sampler: sampler;

// Color profile matrices
const TO_SRGB: mat3x3<f32> = mat3x3<f32>(
    vec3<f32>(3.240970, -0.969244, 0.055630),
    vec3<f32>(-1.537383, 1.875968, -0.203977),
    vec3<f32>(-0.498611, 0.041555, 1.056972)
);

const PROFILE0: mat3x3<f32> = mat3x3<f32>(
    vec3<f32>(0.412391, 0.212639, 0.019331),
    vec3<f32>(0.357584, 0.715169, 0.119195),
    vec3<f32>(0.180481, 0.072192, 0.950532)
);

// Color temperature adjustment matrices
const D65_TO_D55: mat3x3<f32> = mat3x3<f32>(
    vec3<f32>(0.4850339153, 0.2500956126, 0.0227359648),
    vec3<f32>(0.3488957224, 0.6977914447, 0.1162985741),
    vec3<f32>(0.1302823568, 0.0521129427, 0.6861537456)
);

const D65_TO_D93: mat3x3<f32> = mat3x3<f32>(
    vec3<f32>(0.3412754080, 0.1759701322, 0.0159972847),
    vec3<f32>(0.3646170520, 0.7292341040, 0.1215390173),
    vec3<f32>(0.2369894093, 0.0947957637, 1.2481442225)
);

fn plant(tar: vec3<f32>, r: f32) -> vec3<f32> {
    let t = max(max(tar.r, tar.g), tar.b) + 0.00001;
    return tar * r / t;
}

fn contrast(x: f32) -> f32 {
    let y = 2.0 * x - 1.0;
    let s = (sin(y * 1.57079632679) + 1.0) * 0.5;
    return mix(x, s, CONTR);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;

    var img_color = textureSample(source_texture, source_sampler, uv).rgb;
    let aftglow = textureSample(afterglow_texture, afterglow_sampler, uv).rgb;

    // Afterglow blending
    let l = length(aftglow);
    let aftglow_adjusted = AS * normalize(pow(aftglow + vec3<f32>(0.01), vec3<f32>(SAT))) * l;
    let bp = BP / 255.0;

    img_color = min(img_color, vec3<f32>(1.0));
    var color = img_color;

    // Apply color profile transformation
    let p = 2.2;
    color = pow(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(p));
    color = PROFILE0 * color;
    color = TO_SRGB * color;
    color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
    color = pow(color, vec3<f32>(1.0 / p));

    // Skip color profile if CP == -1
    if CP < -0.5 {
        color = img_color;
    }

    // Saturation adjustment
    let luma = dot(color, vec3<f32>(0.299, 0.587, 0.114));
    let scolor2 = mix(vec3<f32>(luma), color, WP_SATURATION);
    color = scolor2;

    // Contrast
    color = plant(color, contrast(max(max(color.r, color.g), color.b)));

    // Gamma
    color = pow(color, vec3<f32>(2.2));
    color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));

    // Color temperature
    var warmer = D65_TO_D55 * color;
    warmer = TO_SRGB * warmer;
    var cooler = D65_TO_D93 * color;
    cooler = TO_SRGB * cooler;

    let m = abs(WP) / 100.0;
    var comp = warmer;
    if WP < 0.0 {
        comp = cooler;
    }
    color = mix(color, comp, m);
    color = pow(max(color, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2));

    // Add afterglow and black point
    if BP > -0.5 {
        color = color + aftglow_adjusted + bp;
    } else {
        color = max(color + BP / 255.0, vec3<f32>(0.0)) / (1.0 + BP / 255.0 * step(-BP / 255.0, max(max(color.r, color.g), color.b))) + aftglow_adjusted;
    }

    color = min(color * PRE_BB, vec3<f32>(1.0));

    return vec4<f32>(color, 1.0);
}
