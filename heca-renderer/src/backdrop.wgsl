// Backdrop sampler: draw a region of a source texture into a destination rect.
//
// The "draw blurred backdrop into a rect" stage that pairs with `blur.rs`: a pane
// or chrome region samples a slice of the (blurred) scene texture and stamps it
// into its own rect as a frosted backdrop, then draws translucent content over it.
// A 6-vertex quad is generated from NDC corners in the uniform — no vertex buffer.

struct Params {
    // Destination rect in NDC: (left, top) and (right, bottom). top > bottom (y up).
    dst_min: vec2<f32>,
    dst_max: vec2<f32>,
    // Source rect in UV space (0..1), origin top-left: (left, top) and (right, bottom).
    uv_min: vec2<f32>,
    uv_max: vec2<f32>,
    // Multiplied into the sampled alpha (0 = invisible, 1 = full).
    opacity: f32,
    _pad: vec3<f32>,
};

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_samp: sampler;
@group(0) @binding(2) var<uniform> p: Params;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    // Two triangles → corners as (tx, ty) in {0,1}², ty=0 is the top edge.
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let c = corners[vi];
    var out: VsOut;
    out.pos = vec4<f32>(
        mix(p.dst_min.x, p.dst_max.x, c.x),
        mix(p.dst_min.y, p.dst_max.y, c.y),
        0.0,
        1.0,
    );
    out.uv = mix(p.uv_min, p.uv_max, c);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var color = textureSample(src_tex, src_samp, in.uv);
    color.a = color.a * p.opacity;
    return color;
}
