/*
   CRT Deconvergence (final pass) - WGSL port

   Original: Copyright (C) 2018-2022 guest(r) - guest.r@gmail.com
   License: GPL-3.0-or-later
*/

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

// Brightness settings
const GLOW: f32 = 0.08;
const BLOOM: f32 = 0.0;
const MASK_BLOOM: f32 = 0.0;
const BLOOM_DIST: f32 = 0.0;
const HALATION: f32 = 0.0;
const GAMMA_C: f32 = 1.0;
const BRIGHT_BOOST: f32 = 1.40;
const BRIGHT_BOOST1: f32 = 1.10;

// Screen options
const IOS: f32 = 0.0;
const CSIZE: f32 = 0.0;
const BSIZE1: f32 = 0.01;
const SBORDER: f32 = 0.75;
const BAR_SPEED: f32 = 50.0;
const BAR_INTENSITY: f32 = 0.1;
const BAR_DIR: f32 = 0.0;
const WARP_X: f32 = 0.03;
const WARP_Y: f32 = 0.04;
const C_SHAPE: f32 = 0.25;
const OVERSCAN_X: f32 = 0.0;
const OVERSCAN_Y: f32 = 0.0;

// Mask options
const SHADOW_MASK: i32 = 0;
const MASK_STR: f32 = 0.3;
const MCUT: f32 = 1.10;
const MASK_SIZE: f32 = 1.0;
const MASK_DARK: f32 = 0.5;
const MASK_LIGHT: f32 = 1.5;
const M_SHIFT: f32 = 0.0;
const MASK_LAYOUT: f32 = 0.0;
const MASK_GAMMA: f32 = 2.40;
const SLOT_MASK: f32 = 0.0;
const SLOT_MASK1: f32 = 0.0;
const SLOT_WIDTH: f32 = 2.0;
const DOUBLE_SLOT: f32 = 1.0;
const SLOT_MS: f32 = 1.0;
const MCLIP: f32 = 0.50;
const GAMMA_OUT: f32 = 1.75;

// Deconvergence
const DCTYPE_X: f32 = 0.0;
const DCTYPE_Y: f32 = 0.0;
const DECONRR: f32 = 0.0;
const DECONRG: f32 = 0.0;
const DECONRB: f32 = 0.0;
const DECONRRY: f32 = 0.0;
const DECONRGY: f32 = 0.0;
const DECONRBY: f32 = 0.0;
const DECONS: f32 = 1.0;

// Noise
const ADD_NOISED: f32 = 0.0;
const NOISE_RESD: f32 = 2.0;
const NOISE_TYPE: f32 = 0.0;

// Other
const POST_BR: f32 = 1.0;
const SCANLINE_WIDTH: f32 = 0.01;
const MAX_SCANLINE_INTENSITY: f32 = 0.6;
const SCANLINE_SHARPNESS: f32 = 0.75;

const EPS: f32 = 1e-10;

@group(2) @binding(0) var source_texture: texture_2d<f32>;
@group(2) @binding(1) var source_sampler: sampler;
@group(2) @binding(2) var<uniform> texture_size: vec2<f32>;
@group(2) @binding(3) var linearize_texture: texture_2d<f32>;
@group(2) @binding(4) var linearize_sampler: sampler;
@group(2) @binding(5) var bloom_texture: texture_2d<f32>;
@group(2) @binding(6) var bloom_sampler: sampler;
@group(2) @binding(7) var pre_texture: texture_2d<f32>;
@group(2) @binding(8) var pre_sampler: sampler;
@group(2) @binding(9) var<uniform> frame_count: u32;
@group(2) @binding(10) var<uniform> source_size: vec2<f32>;

// Configurable CRT parameters from Rust (field order must match Rust struct)
struct CrtParams {
    curvature: vec2<f32>,        // x, y warp amounts
    scanline: vec2<f32>,         // intensity, sharpness
    mask: vec2<f32>,             // strength, type
    glow_brightness: vec2<f32>,  // glow, brightness
    gamma_corner: vec2<f32>,     // gamma_out, corner_size
    enabled: u32,                // 1 = on, 0 = bypass
}
@group(2) @binding(11) var<uniform> crt_params: CrtParams;

fn warp(pos: vec2<f32>) -> vec2<f32> {
    let p = pos * 2.0 - 1.0;
    let warped = vec2<f32>(
        p.x * inverseSqrt(1.0 - C_SHAPE * p.y * p.y),
        p.y * inverseSqrt(1.0 - C_SHAPE * p.x * p.x)
    );
    // Use configurable curvature from uniforms
    let warp_amount = crt_params.curvature;
    let result = mix(p, warped, warp_amount / C_SHAPE);
    return result * 0.5 + 0.5;
}

fn overscan(pos: vec2<f32>, dx: f32, dy: f32) -> vec2<f32> {
    let p = pos * 2.0 - 1.0;
    return p * vec2<f32>(dx, dy) * 0.5 + 0.5;
}

fn humbar(pos: f32, frame: u32) -> f32 {
    if BAR_INTENSITY == 0.0 {
        return 1.0;
    }
    var p = pos;
    if BAR_INTENSITY < 0.0 {
        p = 1.0 - pos;
    } else {
        p = pos;
    }
    p = fract(p + (f32(frame) % BAR_SPEED) / (BAR_SPEED - 1.0));
    if BAR_INTENSITY < 0.0 {
        p = p;
    } else {
        p = 1.0 - p;
    }
    return (1.0 - abs(BAR_INTENSITY)) + abs(BAR_INTENSITY) * p;
}

fn corner(pos: vec2<f32>, output_size: vec4<f32>) -> f32 {
    // Use configurable corner size from uniforms
    let corner_size = crt_params.gamma_corner.y;
    let b = vec2<f32>(corner_size, corner_size) * vec2<f32>(1.0, output_size.x / output_size.y) * 0.05;
    var p = clamp(pos, vec2<f32>(0.0), vec2<f32>(1.0));
    p = abs(2.0 * (p - 0.5));

    var csize1 = mix(400.0, 7.0, pow(4.0 * CSIZE, 0.10));
    var crn = dot(pow(p, vec2<f32>(csize1)), vec2<f32>(1.0, output_size.y / output_size.x));
    if CSIZE == 0.0 {
        crn = max(p.x, p.y);
    } else {
        crn = pow(crn, 1.0 / csize1);
    }
    p = max(p, vec2<f32>(crn));

    var res: vec2<f32>;
    if corner_size == 0.0 {
        res = vec2<f32>(1.0);
    } else {
        res = mix(vec2<f32>(0.0), vec2<f32>(1.0), smoothstep(vec2<f32>(1.0), vec2<f32>(1.0) - b, sqrt(p)));
    }
    res = pow(res, vec2<f32>(SBORDER));
    return sqrt(res.x * res.y);
}

fn plant(tar: vec3<f32>, r: f32) -> vec3<f32> {
    let t = max(max(tar.r, tar.g), tar.b) + 0.00001;
    return tar * r / t;
}

fn declip(c: vec3<f32>, b: f32) -> vec3<f32> {
    let m = max(max(c.r, c.g), c.b);
    if m > b {
        return c * b / m;
    }
    return c;
}

// Shadow mask - pos_in is in screen pixels, scale to align with game pixels
fn mask_fn(pos_in: vec2<f32>, mx: f32) -> vec3<f32> {
    // Use configurable mask strength and type from uniforms
    let mask_str = crt_params.mask.x;
    let mask_type = i32(crt_params.mask.y);

    // Scale screen coords to game pixel coords for alignment
    let pixel_scale = texture_size / source_size;
    var pos = pos_in / pixel_scale;
    var pos0 = pos;
    pos.y = floor(pos.y / MASK_SIZE);
    let next_line = select(0.0, 1.0, fract(pos.y * 0.5) > 0.25);
    if M_SHIFT > -0.25 {
        pos0.x = pos0.x + next_line * M_SHIFT;
    } else {
        pos0.x = pos0.x + pos.y * M_SHIFT;
    }
    pos = floor(pos0 / MASK_SIZE);

    var mask = vec3<f32>(MASK_DARK);
    let one = vec3<f32>(1.0);
    let dark_compensate = mix(max(clamp(mix(MCUT, mask_str, mx), 0.0, 1.0) - 0.4, 0.0) + 1.0, 1.0, mx);
    let mc = 1.0 - max(mask_str, 0.0);

    // Phosphor mask (type 0)
    if mask_type == 0 {
        let px = fract(pos.x * 0.5);
        if px < 0.49 {
            mask = vec3<f32>(1.0, mc, 1.0);
        } else {
            mask = vec3<f32>(mc, 1.0, mc);
        }
    }
    // Aperture-grille (type 2)
    else if mask_type == 2 {
        let px = fract(pos.x / 3.0);
        if px < 0.3 {
            mask.r = MASK_LIGHT;
        } else if px < 0.6 {
            mask.g = MASK_LIGHT;
        } else {
            mask.b = MASK_LIGHT;
        }
    }
    // Trinitron mask 6
    else if mask_type == 6 {
        mask = vec3<f32>(0.0);
        let px = fract(pos.x / 3.0);
        if px < 0.3 {
            mask.r = 1.0;
        } else if px < 0.6 {
            mask.g = 1.0;
        } else {
            mask.b = 1.0;
        }
        mask = clamp(mix(mix(one, mask, MCUT), mix(one, mask, mask_str), mx), vec3<f32>(0.0), vec3<f32>(1.0)) * dark_compensate;
    }
    // No mask (type -1)
    else if mask_type == -1 {
        mask = vec3<f32>(1.0);
    }
    // Default to phosphor
    else {
        let px = fract(pos.x * 0.5);
        if px < 0.49 {
            mask = vec3<f32>(1.0, mc, 1.0);
        } else {
            mask = vec3<f32>(mc, 1.0, mc);
        }
    }

    return mask;
}

fn slot_mask_fn(pos_in: vec2<f32>, m: f32) -> f32 {
    if SLOT_MASK + SLOT_MASK1 == 0.0 {
        return 1.0;
    }

    // Scale screen coords to game pixel coords for alignment
    let pixel_scale = texture_size / source_size;
    let pos = floor((pos_in / pixel_scale) / SLOT_MS);
    let mlen = SLOT_WIDTH * 2.0;
    let px = fract(pos.x / mlen);
    let py = floor(fract(pos.y / (2.0 * DOUBLE_SLOT)) * 2.0 * DOUBLE_SLOT);
    let slot_dark = mix(1.0 - SLOT_MASK1, 1.0 - SLOT_MASK, m);

    var slot = 1.0;
    if py == 0.0 && px < 0.5 {
        slot = slot_dark;
    } else if py == DOUBLE_SLOT && px >= 0.5 {
        slot = slot_dark;
    }

    return slot;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;

    // Bypass CRT effect when disabled - pass through pre-shader output directly
    if crt_params.enabled == 0u {
        return textureSample(pre_texture, pre_sampler, uv);
    }

    let inv_text_size = 1.0 / texture_size;
    let original_size = vec4<f32>(texture_size.x, texture_size.y, inv_text_size.x, inv_text_size.y);
    let output_size = original_size; // Same for simplicity

    let gamma_in = 1.0 / textureSample(linearize_texture, linearize_sampler, vec2<f32>(0.25, 0.25)).a;
    let intera = textureSample(linearize_texture, linearize_sampler, vec2<f32>(0.75, 0.25)).a;
    let interb = intera < 0.5;

    var texcoord = uv;
    texcoord = overscan(texcoord, (original_size.x - OVERSCAN_X) / original_size.x, (original_size.y - OVERSCAN_Y) / original_size.y);

    let pos1 = uv;
    let pos = warp(texcoord);
    let pos0 = warp(uv);

    let color0 = textureSample(source_texture, source_sampler, pos1).rgb;
    let c0 = max(max(color0.r, color0.g), color0.b);

    // Color and bloom fetching
    var color = textureSample(source_texture, source_sampler, pos1).rgb;
    var bloom = textureSample(bloom_texture, bloom_sampler, pos).rgb;

    let cm = max(max(color.r, color.g), color.b);
    let mx1 = textureSample(source_texture, source_sampler, pos1).a;
    let colmx = max(mx1, cm);
    let w3 = min((c0 + 0.0005) / (pow(colmx, gamma_in / 1.4) + 0.0005), 1.0);

    let dx = vec2<f32>(0.001, 0.0);
    let mx0 = textureSample(source_texture, source_sampler, pos1 - dx).a;
    let mx2 = textureSample(source_texture, source_sampler, pos1 + dx).a;
    let mx = max(max(mx0, mx1), max(mx2, cm));

    let one = vec3<f32>(1.0);

    // Apply mask
    let orig1 = color;
    var cmask = one;

    let maskcoord = in.position.xy * 1.000001;
    let smask = slot_mask_fn(maskcoord, mx);
    cmask = cmask * mask_fn(maskcoord, mx);

    if MASK_LAYOUT > 0.5 {
        cmask = cmask.rbg;
    }

    let cmask1 = cmask;
    let smask1 = smask;

    color = pow(color, vec3<f32>(MASK_GAMMA / gamma_in));
    color = color * cmask;
    color = min(color, vec3<f32>(1.0));
    color = color * smask;
    color = pow(color, vec3<f32>(gamma_in / MASK_GAMMA));

    // Use configurable brightness from uniforms
    let brightness = crt_params.glow_brightness.y;
    let bb = mix(brightness, brightness * 0.8, colmx);
    color = color * bb;

    // Glow
    let glow_sample = textureSample(bloom_texture, bloom_sampler, pos).rgb;
    let ref_sample = textureSample(linearize_texture, linearize_sampler, pos).rgb;
    let maxb = textureSample(bloom_texture, bloom_sampler, pos).a;
    let vig = textureSample(pre_texture, pre_sampler, clamp(pos, 0.5 * original_size.zw, 1.0 - 0.5 * original_size.zw)).a;

    var bloom1 = bloom;
    if BLOOM < -0.01 {
        bloom1 = plant(bloom, maxb);
    }
    bloom1 = min(bloom1 * (orig1 + color), max(0.5 * (colmx + orig1 - color), 0.001 * bloom1));
    bloom1 = 0.5 * (bloom1 + mix(bloom1, mix(colmx * orig1, bloom1, 0.5), 1.0 - color));
    bloom1 = bloom1 * mix(1.0, 2.0 - colmx, BLOOM_DIST);

    color = color + abs(BLOOM) * bloom1;
    color = min(color, mix(one, cmask1 * smask1, MCLIP));

    if !interb {
        color = declip(color, mix(1.0, w3, 0.6));
    }

    // Glow application - use configurable glow from uniforms
    let glow_amount = crt_params.glow_brightness.x;
    var glow_color = mix(glow_sample, 0.25 * color, 0.7 * colmx);
    if glow_amount >= 0.0 {
        color = color + 0.5 * glow_color * glow_amount;
    } else {
        var cmask_sq = cmask * smask;
        cmask_sq = cmask_sq * cmask_sq;
        cmask_sq = cmask_sq * cmask_sq;
        color = color + (-glow_amount) * cmask_sq * glow_color;
    }

    color = min(color, vec3<f32>(1.0));
    // Use configurable gamma from uniforms
    let gamma_out = crt_params.gamma_corner.x;
    color = pow(color, vec3<f32>(1.0 / gamma_out));

    // Scanlines - aligned to game pixels, use configurable intensity/sharpness
    let scanline_max_intensity = crt_params.scanline.x;
    let scanline_sharpness = crt_params.scanline.y;
    let game_pixel_y = uv.y * source_size.y;
    let scanline_fract = fract(game_pixel_y);
    let scanline_intensity_linear = max(abs(4.0 * scanline_fract - 2.0) - 1.0, 0.0);
    let scanline_intensity_full = smoothstep(0.0, 1.0, pow(scanline_intensity_linear, scanline_sharpness));
    let scanline_intensity = scanline_intensity_full * scanline_max_intensity;

    // Hum bar and corner
    var bar_pos = pos.y;
    if BAR_DIR > 0.5 {
        bar_pos = pos.x;
    }
    var c = color * (1.0 - scanline_intensity) * humbar(bar_pos, frame_count) * POST_BR * corner(pos0, output_size);

    return vec4<f32>(c, 1.0);
}
