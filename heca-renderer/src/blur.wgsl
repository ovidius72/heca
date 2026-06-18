// In-app Kawase blur.
//
// A single separable Gaussian undersamples a large radius (taps `span` px apart),
// so narrow high-frequency content — a 2px cursor, fine text strokes — aliases
// through as visible lines, and coarse features survive because the radius is too
// small to smear them. The Kawase filter instead runs N passes with an increasing
// offset: each pass is a weighted 9-tap (center ×4 + 4 cardinal ×2 + 4 diagonal
// ×1, ÷16) at a single offset `d` texels, and `d` grows from small to the target
// radius across passes. Early small-offset passes smooth fine detail so later
// large-offset passes blur already-smoothed content (no aliasing), and the
// compounding offsets yield a smooth strong blur (σ ≈ √Σdᵢ²) — the standard
// frosted-glass technique. A single oversized triangle covers the screen (no
// vertex buffer); the offset comes from a uniform so one pipeline serves all
// passes.

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
    // Kawase offset in UV units: (offset/width, offset/height). 0 = passthrough.
    step: vec2<f32>,
    // Blur radius in source pixels (0 = passthrough gate; the offset drives the
    // actual spread — radius only selects pass vs. passthrough).
    radius: f32,
    _pad: f32,
};

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_samp: sampler;
@group(0) @binding(2) var<uniform> params: BlurParams;

// Kawase 9-tap weights: center ×4, 4 cardinal ×2, 4 diagonal ×1 → total 16.
@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    if (params.radius <= 0.0) {
        return textureSample(src_tex, src_samp, in.uv);
    }
    let o = params.step;
    var acc: vec4<f32> = textureSample(src_tex, src_samp, in.uv) * 4.0;
    // 4 cardinal taps (±offset along one axis).
    acc += textureSample(src_tex, src_samp, in.uv + vec2<f32>(o.x, 0.0)) * 2.0;
    acc += textureSample(src_tex, src_samp, in.uv - vec2<f32>(o.x, 0.0)) * 2.0;
    acc += textureSample(src_tex, src_samp, in.uv + vec2<f32>(0.0, o.y)) * 2.0;
    acc += textureSample(src_tex, src_samp, in.uv - vec2<f32>(0.0, o.y)) * 2.0;
    // 4 diagonal taps (±offset along both axes).
    acc += textureSample(src_tex, src_samp, in.uv + o);
    acc += textureSample(src_tex, src_samp, in.uv + vec2<f32>(o.x, -o.y));
    acc += textureSample(src_tex, src_samp, in.uv - o);
    acc += textureSample(src_tex, src_samp, in.uv + vec2<f32>(-o.x, o.y));
    return acc / 16.0;
}