// Grid UI primitive shader: SDF rounded rectangle with border + additive neon glow.
//
// Each rect is drawn as a quad expanded to fit its glow halo. The fragment
// shader evaluates a signed distance to the rounded box, then composes:
//   inside        -> fill (+ border band near the edge)
//   outside (d>0) -> exponential glow falloff (additive-ish)
// All effects are resolution-independent (computed per fragment in logical px).

struct Uniforms {
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> u: Uniforms;

struct VertexInput {
    @location(0) position: vec2<f32>,      // expanded quad corner, logical px
    @location(1) center: vec2<f32>,        // rect center, logical px
    @location(2) half_size: vec2<f32>,     // rect half-extent, logical px
    @location(3) radius: f32,              // corner radius
    @location(4) fill: vec4<f32>,
    @location(5) border: vec4<f32>,
    @location(6) border_width: f32,
    @location(7) glow: vec4<f32>,
    @location(8) glow_radius: f32,
    @location(9) glow_intensity: f32,
    @location(10) shadow: vec4<f32>,
    @location(11) shadow_radius: f32,
    @location(12) shadow_offset: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec2<f32>,
    @location(1) center: vec2<f32>,
    @location(2) half_size: vec2<f32>,
    @location(3) radius: f32,
    @location(4) fill: vec4<f32>,
    @location(5) border: vec4<f32>,
    @location(6) border_width: f32,
    @location(7) glow: vec4<f32>,
    @location(8) glow_radius: f32,
    @location(9) glow_intensity: f32,
    @location(10) shadow: vec4<f32>,
    @location(11) shadow_radius: f32,
    @location(12) shadow_offset: vec2<f32>,
};

fn srgb_to_linear_channel(v: f32) -> f32 {
    if (v <= 0.04045) {
        return v / 12.92;
    }
    return pow((v + 0.055) / 1.055, 2.4);
}

fn srgb_to_linear(rgb: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        srgb_to_linear_channel(rgb.r),
        srgb_to_linear_channel(rgb.g),
        srgb_to_linear_channel(rgb.b),
    );
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let x = (in.position.x / u.screen_size.x) * 2.0 - 1.0;
    let y = -((in.position.y / u.screen_size.y) * 2.0 - 1.0);
    out.clip = vec4<f32>(x, y, 0.0, 1.0);
    out.world = in.position;
    out.center = in.center;
    out.half_size = in.half_size;
    out.radius = in.radius;
    out.fill = in.fill;
    out.border = in.border;
    out.border_width = in.border_width;
    out.glow = in.glow;
    out.glow_radius = in.glow_radius;
    out.glow_intensity = in.glow_intensity;
    out.shadow = in.shadow;
    out.shadow_radius = in.shadow_radius;
    out.shadow_offset = in.shadow_offset;
    return out;
}

// Signed distance to a rounded box centered at the origin.
fn sd_round_box(p: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let rr = min(r, min(b.x, b.y));
    let q = abs(p) - b + vec2<f32>(rr, rr);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0, 0.0))) - rr;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = in.world - in.center;
    let d = sd_round_box(p, in.half_size, in.radius);
    // Resolution-independent antialiasing: ~1 physical pixel wide regardless of
    // DPI or zoom (fwidth gives the change in `d` per fragment).
    let fw = max(fwidth(d), 1e-5);

    // Straight (non-premultiplied) surface color + coverage alpha.
    let inside = clamp(0.5 - d / fw, 0.0, 1.0);
    var rgb = srgb_to_linear(in.fill.rgb);
    var a = in.fill.a * inside;

    // Border: a band of width `border_width` just inside the edge.
    if (in.border_width > 0.0 && in.border.a > 0.0) {
        let outer = clamp(0.5 - d / fw, 0.0, 1.0);
        let inner = clamp(0.5 - (d + in.border_width) / fw, 0.0, 1.0);
        let band = clamp(outer - inner, 0.0, 1.0);
        rgb = mix(rgb, srgb_to_linear(in.border.rgb), band);
        a = max(a, in.border.a * band);
    }

    // Premultiply for premultiplied-alpha blending (src = One, dst = 1-srcA).
    var out_rgb = rgb * a;
    var out_a = a;

    // Drop shadow: a soft, dark, OFFSET halo cast BEHIND the shape. Unlike the
    // glow (additive light) it composites a dark color WITH alpha, so it darkens
    // the background and reads on dark themes. Sampled from the shape's SDF
    // shifted by `shadow_offset`, with a quadratic falloff over `shadow_radius`,
    // then composited UNDER the shape ("shape over shadow") so the opaque shape
    // hides the shadow's center and only its fringe shows.
    if (in.shadow_radius > 0.0 && in.shadow.a > 0.0) {
        let ds = sd_round_box(p - in.shadow_offset, in.half_size, in.radius);
        let ts = clamp(1.0 - max(ds, 0.0) / in.shadow_radius, 0.0, 1.0);
        let sa = ts * ts * in.shadow.a;
        let inv = 1.0 - out_a;
        out_rgb = out_rgb + in.shadow.rgb * (sa * inv);
        out_a = out_a + sa * inv;
    }

    // Glow: additive light with a smooth compact falloff (0 by `glow_radius`,
    // so it fully fades inside the expanded quad — no hard cutoff). It adds
    // color but NOT alpha, so it brightens the background through itself like a
    // real glow instead of painting a solid block.
    if (in.glow_radius > 0.0 && in.glow_intensity > 0.0 && d > 0.0) {
        let t = clamp(1.0 - d / in.glow_radius, 0.0, 1.0);
        let g = t * t * in.glow_intensity * in.glow.a;
        out_rgb = out_rgb + srgb_to_linear(in.glow.rgb) * g;
    }

    return vec4<f32>(out_rgb, out_a);
}

// Stencil-write fragment: mark only fragments INSIDE the rounded rect. Used by the
// rounded content-clip mechanism — `render_stencil` draws each pane's inner
// rounded rect here with `StencilOp::Replace`, then the content renderers
// (backdrop/primitive/text) test against the mask so terminal content follows the
// pane's rounded border instead of poking past it at high corner radii. There are
// no color targets on the stencil pipeline, so this shader writes nothing to color;
// `discard`-ing outside the shape leaves the stencil untouched there.
@fragment
fn fs_stencil(in: VertexOutput) {
    let p = in.world - in.center;
    let d = sd_round_box(p, in.half_size, in.radius);
    if (d > 0.0) {
        discard;
    }
}
