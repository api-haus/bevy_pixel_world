// CRT Common Definitions
// Shared constants and utilities for CRT shaders

// Vertex output structure shared by all CRT passes
struct CrtVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// Standard fullscreen vertex shader for CRT passes
// Uses Bevy's mesh2d bindings
#import bevy_sprite::mesh2d_vertex_output::VertexOutput as Mesh2dVertexOutput

// CRT parameters - these could be made configurable via uniforms
const GAMMA_INPUT: f32 = 1.8;
const GAMMA_OUT: f32 = 1.75;

// Phosphor persistence
const PR: f32 = 0.12;
const PG: f32 = 0.12;
const PB: f32 = 0.12;
const AS: f32 = 0.20;  // Afterglow strength
const SAT: f32 = 0.50; // Afterglow saturation

// Curvature
const WARP_X: f32 = 0.03;
const WARP_Y: f32 = 0.04;
const C_SHAPE: f32 = 0.25;

// Scanlines
const SCANLINE_WIDTH: f32 = 0.01;
const MAX_SCANLINE_INTENSITY: f32 = 0.6;
const SCANLINE_SHARPNESS: f32 = 0.75;

// Mask
const SHADOW_MASK: i32 = 0;
const MASK_SIZE: f32 = 1.0;
const MASK_DARK: f32 = 0.5;
const MASK_LIGHT: f32 = 1.5;
const MASK_STRENGTH: f32 = 0.3;

// Bloom
const GLOW: f32 = 0.08;
const BLOOM: f32 = 0.0;

// Brightness
const BRIGHT_BOOST: f32 = 1.40;
const BRIGHT_BOOST1: f32 = 1.10;

// Helper: apply curvature warp to UV coordinates
fn warp(pos: vec2<f32>) -> vec2<f32> {
    let p = pos * 2.0 - 1.0;
    let warped = vec2<f32>(
        p.x * inverseSqrt(1.0 - C_SHAPE * p.y * p.y),
        p.y * inverseSqrt(1.0 - C_SHAPE * p.x * p.x)
    );
    let result = mix(p, warped, vec2<f32>(WARP_X, WARP_Y) / C_SHAPE);
    return result * 0.5 + 0.5;
}

// Helper: overscan adjustment
fn overscan(pos: vec2<f32>, dx: f32, dy: f32) -> vec2<f32> {
    let p = pos * 2.0 - 1.0;
    return p * vec2<f32>(dx, dy) * 0.5 + 0.5;
}

// Helper: normalize color while preserving target brightness
fn plant(tar: vec3<f32>, r: f32) -> vec3<f32> {
    let t = max(max(tar.r, tar.g), tar.b) + 0.00001;
    return tar * r / t;
}

// Gaussian function for blur passes
fn gaussian(x: f32, sigma: f32) -> f32 {
    let inv_sigma_sq = 1.0 / (2.0 * sigma * sigma);
    return exp(-x * x * inv_sigma_sq);
}
