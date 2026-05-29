struct Uniforms {
    screen_size: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var text_texture: texture_2d<f32>;
@group(0) @binding(2)
var text_sampler: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) texcoord: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) texcoord: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let x = (in.position.x / uniforms.screen_size.x) * 2.0 - 1.0;
    let y = -((in.position.y / uniforms.screen_size.y) * 2.0 - 1.0);
    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    out.texcoord = in.texcoord;
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let alpha = textureSample(text_texture, text_sampler, in.texcoord).r;
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
