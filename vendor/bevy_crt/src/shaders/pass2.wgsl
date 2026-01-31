/*
   CRT Pass2 - Vertical filtering + scanlines - WGSL port

   Original: Copyright (C) 2018-2021 guest(r) - guest.r@gmail.com
   License: GPL-3.0-or-later
*/

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

// Vertical filtering parameters
const V_SHARPNESS: f32 = 1.0;
const SIGMA_VER: f32 = 0.50;
const S_SHARP_V: f32 = 1.0;
const V_SHARP: f32 = 1.25;
const V_ARNG: f32 = 0.2;
const INTERNAL_RES: f32 = 1.0;
const PRESCALE_X: f32 = 1.0;
const PRESCALE_Y: f32 = 1.0;

// Screen options
const INTRES: f32 = 0.0;
const IOS: f32 = 0.0;
const WARP_X: f32 = 0.03;
const WARP_Y: f32 = 0.04;
const C_SHAPE: f32 = 0.25;
const OVERSCAN_X: f32 = 0.0;
const OVERSCAN_Y: f32 = 0.0;

// Brightness
const GAMMA_C: f32 = 1.0;
const BRIGHT_BOOST: f32 = 1.4;
const BRIGHT_BOOST1: f32 = 1.1;

// Scanline parameters
const GSL: f32 = 0.0;
const SCANLINE1: f32 = 6.0;
const SCANLINE2: f32 = 8.0;
const BEAM_MIN: f32 = 1.2;
const BEAM_MAX: f32 = 1.0;
const BEAM_SIZE: f32 = 0.6;
const VERT_MASK: f32 = 0.0;
const SCANS: f32 = 0.6;
const SCAN_FALLOFF: f32 = 1.0;
const SCAN_GAMMA: f32 = 2.4;

const EPS: f32 = 1e-10;

@group(2) @binding(0) var pass1_texture: texture_2d<f32>;
@group(2) @binding(1) var pass1_sampler: sampler;
@group(2) @binding(2) var<uniform> texture_size: vec2<f32>;
@group(2) @binding(3) var linearize_texture: texture_2d<f32>;
@group(2) @binding(4) var linearize_sampler: sampler;

fn warp(pos: vec2<f32>) -> vec2<f32> {
    let p = pos * 2.0 - 1.0;
    let warped = vec2<f32>(
        p.x * inverseSqrt(1.0 - C_SHAPE * p.y * p.y),
        p.y * inverseSqrt(1.0 - C_SHAPE * p.x * p.x)
    );
    let result = mix(p, warped, vec2<f32>(WARP_X, WARP_Y) / C_SHAPE);
    return result * 0.5 + 0.5;
}

fn overscan(pos: vec2<f32>, dx: f32, dy: f32) -> vec2<f32> {
    let p = pos * 2.0 - 1.0;
    return p * vec2<f32>(dx, dy) * 0.5 + 0.5;
}

fn st(x: f32) -> f32 {
    return exp2(-10.0 * x * x);
}

fn sw0(x: f32, color: f32, scanline: f32) -> f32 {
    let tmp = mix(BEAM_MIN, BEAM_MAX, color);
    let ex = x * tmp;
    var ex_sq = ex * ex;
    if GSL <= -0.5 {
        ex_sq = mix(ex_sq, ex_sq * ex, 0.4);
    }
    return exp2(-scanline * ex_sq);
}

fn sw1(x: f32, color: f32, scanline: f32) -> f32 {
    let x_adj = mix(x, BEAM_MIN * x, max(x - 0.4 * color, 0.0));
    let tmp = mix(1.2 * BEAM_MIN, BEAM_MAX, color);
    let ex = x_adj * tmp;
    return exp2(-scanline * ex * ex);
}

fn sw2(x: f32, color: f32, scanline: f32) -> f32 {
    var tmp = mix((2.5 - 0.5 * color) * BEAM_MIN, BEAM_MAX, color);
    tmp = mix(BEAM_MAX, tmp, pow(x, color + 0.3));
    let ex = x * tmp;
    return exp2(-scanline * ex * ex);
}

fn gc(c: vec3<f32>) -> vec3<f32> {
    let mc = max(max(c.r, c.g), c.b);
    let mg = pow(mc, 1.0 / GAMMA_C);
    return c * mg / (mc + EPS);
}

fn gaussian(x: f32) -> f32 {
    let inv_sigma_sq = 1.0 / (2.0 * SIGMA_VER * SIGMA_VER * INTERNAL_RES * INTERNAL_RES);
    return exp(-x * x * inv_sigma_sq);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;

    let inv_text_size = 1.0 / texture_size;
    let original_size = vec4<f32>(texture_size.x, texture_size.y, inv_text_size.x, inv_text_size.y);
    let o_source_size = original_size * vec4<f32>(PRESCALE_X, PRESCALE_Y, 1.0 / PRESCALE_X, 1.0 / PRESCALE_Y);
    var source_size = vec4<f32>(o_source_size.x, original_size.y, o_source_size.z, original_size.w);

    // Get gamma info from linearize pass
    let gamma_in = 1.0 / textureSample(linearize_texture, linearize_sampler, vec2<f32>(0.25, 0.25)).a;
    let intera = textureSample(linearize_texture, linearize_sampler, vec2<f32>(0.75, 0.25)).a;
    let interb = intera < 0.5;

    var texcoord = uv;
    texcoord = overscan(texcoord, (original_size.x - OVERSCAN_X) / original_size.x, (original_size.y - OVERSCAN_Y) / original_size.y);
    let pos = warp(texcoord);

    let coffset = 0.5;
    let ps = source_size.zw;
    let ogl2_pos = pos.y * source_size.y - coffset;
    let f_raw = fract(ogl2_pos);

    // Anti-alias the fractional position using screen-space derivatives
    let dpy = fwidth(ogl2_pos);
    let aa_blend = clamp(dpy, 0.0, 1.0);
    // When dpy is high (non-integer scaling), blend towards 0.5 to reduce aliasing
    let f = mix(f_raw, 0.5, aa_blend * 0.5);

    let dy = vec2<f32>(0.0, ps.y);

    // Reading texels
    var pc4: vec2<f32>;
    pc4.y = floor(ogl2_pos) * ps.y + 0.5 * ps.y;
    pc4.x = pos.x;

    var color1 = textureSample(pass1_texture, pass1_sampler, pc4).rgb;
    let scolor1 = textureSample(pass1_texture, pass1_sampler, pc4).aaa;
    color1 = pow(color1, vec3<f32>(SCAN_GAMMA / gamma_in));

    let pc4_2 = pc4 + dy;
    var color2 = textureSample(pass1_texture, pass1_sampler, pc4_2).rgb;
    let scolor2 = textureSample(pass1_texture, pass1_sampler, pc4_2).aaa;
    color2 = pow(color2, vec3<f32>(SCAN_GAMMA / gamma_in));

    // Calculating scanlines
    var ctmp = color1;
    var w3 = 1.0;
    var color = color1;
    let one = vec3<f32>(1.0);

    if !interb {
        let shape1 = mix(SCANLINE1, SCANLINE2, f);
        let shape2 = mix(SCANLINE1, SCANLINE2, 1.0 - f);

        let wt1 = st(f);
        let wt2 = st(1.0 - f);

        let color00 = color1 * wt1 + color2 * wt2;
        let scolor0 = scolor1 * wt1 + scolor2 * wt2;

        ctmp = color00 / (wt1 + wt2);
        let sctmp = max(scolor0 / (wt1 + wt2), ctmp);

        let cref1 = mix(sctmp, scolor1, BEAM_SIZE);
        let creff1 = pow(max(max(cref1.r, cref1.g), cref1.b), SCAN_FALLOFF);
        let cref2 = mix(sctmp, scolor2, BEAM_SIZE);
        let creff2 = pow(max(max(cref2.r, cref2.g), cref2.b), SCAN_FALLOFF);

        let f1 = f;
        let f2 = 1.0 - f;

        var wf1: f32;
        var wf2: f32;
        if GSL < 0.5 {
            wf1 = sw0(f1, creff1, shape1);
            wf2 = sw0(f2, creff2, shape2);
        } else if GSL == 1.0 {
            wf1 = sw1(f1, creff1, shape1);
            wf2 = sw1(f2, creff2, shape2);
        } else {
            wf1 = sw2(f1, creff1, shape1);
            wf2 = sw2(f2, creff2, shape2);
        }

        if wf1 + wf2 > 1.0 {
            let wtmp = 1.0 / (wf1 + wf2);
            wf1 = wf1 * wtmp;
            wf2 = wf2 * wtmp;
        }

        var w1 = vec3<f32>(wf1);
        var w2 = vec3<f32>(wf2);
        w3 = wf1 + wf2;

        let mc1 = max(max(color1.r, color1.g), color1.b) + EPS;
        let mc2 = max(max(color2.r, color2.g), color2.b) + EPS;

        var cref1_sat = color1 / mc1;
        cref1_sat = cref1_sat * cref1_sat;
        cref1_sat = cref1_sat * cref1_sat;

        var cref2_sat = color2 / mc2;
        cref2_sat = cref2_sat * cref2_sat;
        cref2_sat = cref2_sat * cref2_sat;

        w1 = max(mix(w1 * mix(one, cref1_sat, SCANS), w1, wf1 * min(1.0 + 0.15 * SCANS, 1.2)), vec3<f32>(0.0));
        w1 = min(w1 * color1, vec3<f32>(mc1)) / (color1 + EPS);

        w2 = max(mix(w2 * mix(one, cref2_sat, SCANS), w2, wf2 * min(1.0 + 0.15 * SCANS, 1.2)), vec3<f32>(0.0));
        w2 = min(w2 * color2, vec3<f32>(mc2)) / (color2 + EPS);

        // Scanline deconvergence
        var cd1 = one;
        var cd2 = one;
        let vm = sqrt(abs(VERT_MASK));

        let v_high1 = 1.0 + 0.3 * vm;
        let v_high2 = 1.0 + 0.6 * vm;
        let v_low = 1.0 - vm;

        let ds1 = min(max(1.0 - w3 * w3, 2.5 * f1), 1.0);
        let ds2 = min(max(1.0 - w3 * w3, 2.5 * f2), 1.0);

        if VERT_MASK < 0.0 {
            cd1 = mix(one, vec3<f32>(v_high2, v_low, v_low), ds1);
            cd2 = mix(one, vec3<f32>(v_low, v_high1, v_high1), ds2);
        } else {
            cd1 = mix(one, vec3<f32>(v_high1, v_low, v_high1), ds1);
            cd2 = mix(one, vec3<f32>(v_low, v_high2, v_low), ds2);
        }

        color = gc(color1) * w1 * cd1 + gc(color2) * w2 * cd2;
        color = min(color, vec3<f32>(1.0));
    }

    if interb {
        color = gc(color1);
    }

    let colmx = pow(max(max(ctmp.r, ctmp.g), ctmp.b), 1.40 / gamma_in);

    if !interb {
        color = pow(color, vec3<f32>(gamma_in / SCAN_GAMMA));
    }

    return vec4<f32>(color, colmx);
}
