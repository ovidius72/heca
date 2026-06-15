// In-app separable Gaussian blur.
//
// Two passes (horizontal then vertical) over a source texture produce a blurred
// copy — the reusable backdrop for frosted chrome / translucent panes. A single
// oversized triangle covers the screen (no vertex buffer). The blur direction and
// strength come from a uniform so one pipeline serves both passes.

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var out: VsOut;
    let x = f32((vi << 1u) & 2u);
    let y = f32(vi & 2u);
    out.uv = vec2<f32>(x, y);                       // (0,0) (2,0) (0,2)
    out.pos = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    return out;
}

struct BlurParams {
    // Per-texel step along the blur axis: (1/width, 0) for horizontal,
    // (0, 1/height) for vertical. Magnitude already scaled by the radius.
    step: vec2<f32>,
    // Blur radius in source pixels (0 = passthrough).
    radius: f32,
    _pad: f32,
};

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_samp: sampler;
@group(0) @binding(2) var<uniform> params: BlurParams;

// 9-tap Gaussian (sigma ≈ 2.0), symmetric — center + 4 each side. Linear
// sampling could halve the taps; kept explicit here for clarity/correctness.
const W0: f32 = 0.2270270270;
const W1: f32 = 0.1945945946;
const W2: f32 = 0.1216216216;
const W3: f32 = 0.0540540541;
const W4: f32 = 0.0162162162;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    if (params.radius <= 0.0) {
        return textureSample(src_tex, src_samp, in.uv);
    }
    let s = params.step;
    var acc: vec4<f32> = textureSample(src_tex, src_samp, in.uv) * W0;
    acc += textureSample(src_tex, src_samp, in.uv + s * 1.0) * W1;
    acc += textureSample(src_tex, src_samp, in.uv - s * 1.0) * W1;
    acc += textureSample(src_tex, src_samp, in.uv + s * 2.0) * W2;
    acc += textureSample(src_tex, src_samp, in.uv - s * 2.0) * W2;
    acc += textureSample(src_tex, src_samp, in.uv + s * 3.0) * W3;
    acc += textureSample(src_tex, src_samp, in.uv - s * 3.0) * W3;
    acc += textureSample(src_tex, src_samp, in.uv + s * 4.0) * W4;
    acc += textureSample(src_tex, src_samp, in.uv - s * 4.0) * W4;
    return acc;
}
