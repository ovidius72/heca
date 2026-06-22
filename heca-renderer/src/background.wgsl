// Background blit — copies the (blurred) z=0 result into the static cache target.
//
// A single oversized triangle covers the screen (no vertex buffer); the fragment
// shader samples the source texture 1:1 into the destination. This snapshots the
// shared `Blur`'s returned `view_b` into `BackgroundLayer`'s own `cache_tex`, so the
// cache stays valid across the rest of the frame (the shared blur is reused for
// the floating-pane frost later in the same frame, which would overwrite
// `view_b`). Linear sampler + ClampToEdge (matches the blur's sampling).

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

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_samp: sampler;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(src_tex, src_samp, in.uv);
}