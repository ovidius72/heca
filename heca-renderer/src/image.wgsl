// Inline terminal-image pipeline: one textured quad per image placement.
//
// Positions arrive in logical pixel coordinates and map to clip space exactly
// like `primitive.wgsl`. The source texture is uploaded as `Rgba8UnormSrgb`, so
// `textureSample` returns linear values (hardware sRGB decode); we premultiply
// by alpha to match the `PREMULTIPLIED_ALPHA_BLENDING` target, then the sRGB
// render target re-encodes on store.

struct Uniforms {
    screen_size: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(1) @binding(0)
var image_tex: texture_2d<f32>;
@group(1) @binding(1)
var image_sampler: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let x = (in.position.x / uniforms.screen_size.x) * 2.0 - 1.0;
    let y = -((in.position.y / uniforms.screen_size.y) * 2.0 - 1.0);
    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let c = textureSample(image_tex, image_sampler, in.uv);
    return vec4<f32>(c.rgb * c.a, c.a);
}
