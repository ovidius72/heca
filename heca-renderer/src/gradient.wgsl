// Gradient fill — z=0 background layer source.
//
// A single oversized triangle covers the screen (no vertex buffer); the fragment
// shader mixes `top` → `bottom` along the screen's vertical axis. `uv.y` is 0 at
// the top edge and 1 at the bottom edge within the visible region (the triangle
// extends past the screen, so it is clamped to [0,1] for safety). Colors are linear
// f32x4 passed via a uniform — no texture, no app types.

struct Params {
    top: vec4<f32>,
    bottom: vec4<f32>,
};

@group(0) @binding(0) var<uniform> p: Params;

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

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let t = clamp(in.uv.y, 0.0, 1.0);
    return mix(p.top, p.bottom, t);
}