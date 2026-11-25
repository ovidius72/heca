// Fullscreen blit: copy the persistent scene texture onto the swapchain.
// A single oversized triangle covers the screen (no vertex buffer needed).

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

@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var scene_samp: sampler;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(scene_tex, scene_samp, in.uv);
}

// The clear pipeline's fragment: a constant 1, which its blend turns into the blend constant —
// so the part of the scene it covers takes exactly the clear colour, whatever was there.
@fragment
fn fs_one() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}
