use cosmic_text::CacheKey;
use cosmic_text::{
    Attrs, Buffer, Family, FeatureTag, FontFeatures, FontSystem, Metrics, Shaping, Style,
    SwashCache, Weight,
};
use heca_grid_ui::scene::TextAlign;
use wgpu::util::DeviceExt;

use crate::font;

use crate::clip::{combine_clip, intersect};
use crate::composite::content_clip_stencil_state;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct TextVertex {
    position: [f32; 2],
    texcoord: [f32; 2],
    color: [f32; 4],
    /// `0.0` = a normal glyph (occludes what is behind it); `1.0` = an additive
    /// halo tap (adds light, writes no alpha). Under premultiplied-alpha blending
    /// a fragment with `a == 0` and `rgb > 0` is exactly additive — the same trick
    /// `grid.wgsl` uses to let a rect's glow add light without occluding.
    additive: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    screen_size: [f32; 2],
    _pad: [f32; 2],
}

pub struct TextStyle<'a> {
    pub color: [f32; 4],
    pub bold: bool,
    pub italic: bool,
    pub faux_italic: bool,
    pub font_family: Option<&'a str>,
}

#[derive(Clone, Copy)]
pub struct TextBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// An additive halo behind a text run, mirroring `heca-grid-ui`'s `Glow` on a rect.
///
/// The scene asks for a glow declaratively; *how* it is realized is this renderer's
/// choice. The glyph atlas stores coverage masks (`R8Unorm`) with one texel of
/// padding, so a multi-tap blur read in the fragment shader would bleed into
/// neighbouring atlas entries — hence the halo is built from offset copies of the
/// run drawn additively beneath the sharp glyphs. Swapping this for a true SDF text
/// pass later would change nothing above this layer.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TextGlow {
    /// Halo colour (alpha is the caller's overall strength).
    pub color: [f32; 4],
    /// Falloff radius in logical pixels — how far the halo reaches.
    pub radius: f32,
    /// Strength multiplier in `0.0..=1.0`.
    pub intensity: f32,
}

impl TextGlow {
    /// Whether this glow actually produces a halo. A glow with no reach, no
    /// strength, or no opacity draws nothing.
    ///
    /// This is the **single** definition of "inert", used both when emitting the
    /// halo and when hashing the cache key. If the two ever disagreed, a run whose
    /// glow draws nothing would still hash differently from an unglowing one and
    /// rebuild its geometry every frame.
    fn is_visible(self) -> bool {
        self.radius > 0.0 && self.intensity > 0.0 && self.color[3] > 0.0
    }
}

/// Blur sigma as a fraction of [`TextGlow::radius`].
///
/// A Gaussian is visually spent by about 3 sigma, so a third of the requested radius
/// puts the halo's visible edge at that radius.
const HALO_SIGMA_FRAC: f32 = 1.0 / 3.0;
/// Strength multiplier on a **dark** background, where the halo composites
/// additively.
///
/// Above `1.0` on purpose. Because the blurred mask is renormalized to full peak,
/// `1.0` would make a glyph's halo peak at exactly a lit surface's — which measures
/// equal but *looks* dimmer, since a glyph emits over a far smaller perimeter than a
/// button does. This compensates for the area a small emitter cannot cover.
const HALO_DARK_GAIN: f32 = 1.6;
/// Strength multiplier on a **light** background, applied on top of the rect
/// shader's own light-background curve (`sqrt(base) * glow_alpha_scale`). Carries the
/// same small-emitter compensation as [`HALO_DARK_GAIN`], so the two themes stay in
/// step when either is retuned.
const HALO_LIGHT_GAIN: f32 = 1.6;

struct TextCommand {
    text: String,
    /// Position: top-left `(x, y)` when `centered` is false, otherwise the box
    /// `(x, y, w, h)` the text is centered within.
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    font_size: f32,
    color: [f32; 4],
    bold: bool,
    italic: bool,
    faux_italic: bool,
    align: TextAlign,
    /// Center within `(w, h)` (grid scene), or place at `(x, y)` (app labels).
    centered: bool,
    /// Align within a stable line box without using the generic UI centered-box path.
    line_box: bool,
    /// Shape with the embedded icon font instead of the text family.
    icon: bool,
    /// Override font family for this run when needed.
    font_family: Option<String>,
    /// Clip rect (logical px) this label is scissored to, if any.
    clip: Option<[f32; 4]>,
    /// Additive halo behind this run, or `None` for flat text. Always `None` for
    /// terminal runs: cell glyphs are the hottest path in the app and a halo would
    /// multiply their vertex count.
    glow: Option<TextGlow>,
    /// Terminal run metadata. When set, the command is one shaped row-run of
    /// terminal cells: the shaper sees the whole run text (so OpenType ligatures
    /// like `->`/`!=` can form), and the emission snaps every glyph back to its
    /// originating cell column so the monospace grid stays exact. `None` for all
    /// UI/chrome labels (the generic align/ink-box path).
    run: Option<TerminalRun>,
}

/// Per-run terminal layout metadata carried alongside a [`TextCommand`].
struct TerminalRun {
    /// Logical cell width; each glyph is snapped to `run_x + col * cell_w`.
    cell_w: f32,
    /// Per-byte column offset within the run text: `byte_cols[b]` is the cell
    /// column (relative to the run start) of the cell that byte `b` belongs to.
    /// Length equals the run text's byte length. Derived from cell display
    /// widths, so it is a pure function of the run's cells.
    byte_cols: Vec<u16>,
    /// Per-column foreground color: `col_colors[col]` tints the glyph snapped to
    /// that column. Lets a ligature span a color boundary while each glyph keeps
    /// its own cell's color.
    col_colors: Vec<[f32; 4]>,
    /// Whether `calt`/`liga`/`clig` are applied. `false` disables them so the run
    /// shapes character-by-character (no ligatures).
    ligatures: bool,
}

/// One label's draw range: its glyph quads occupy `[first_index, first_index +
/// index_count)` of the frame's index buffer, scissored to `clip`.
struct DrawRange {
    clip: Option<[f32; 4]>,
    first_index: u32,
    index_count: u32,
}

/// Convert a logical clip rect to a physical scissor rect clamped to the
/// framebuffer `target`: `(x, y, w, h)`. A zero `w`/`h` means "fully clipped".
fn scissor_px(c: [f32; 4], scale: f32, target: [u32; 2]) -> (u32, u32, u32, u32) {
    let (fw, fh) = (target[0] as f32, target[1] as f32);
    let x0 = (c[0] * scale).clamp(0.0, fw);
    let y0 = (c[1] * scale).clamp(0.0, fh);
    let x1 = ((c[0] + c[2]) * scale).clamp(0.0, fw);
    let y1 = ((c[1] + c[3]) * scale).clamp(0.0, fh);
    (
        x0 as u32,
        y0 as u32,
        (x1 - x0).max(0.0) as u32,
        (y1 - y0).max(0.0) as u32,
    )
}

/// Identity of a shaped label: same text + size + weight + family produces the same
/// glyph layout, so the layout is cached and reused across frames. Color is **not**
/// part of the key — glyph masks are tinted per-draw in the shader (vertex color).
#[derive(Clone, PartialEq, Eq, Hash)]
struct LabelKey {
    text: String,
    /// Scaled font size in `f32::to_bits()` form (exact, hashable).
    size_bits: u32,
    bold: bool,
    icon: bool,
    italic: bool,
    font_family: Option<String>,
    /// Whether `calt`/`liga`/`clig` are applied (terminal runs can disable them).
    /// Always `true` for the generic UI/chrome path (default features on).
    ligatures: bool,
}

/// One placed glyph within a label: its quad (relative to the label's ink-box
/// top-left, physical px) and the atlas region to sample. The glyph bitmaps live in
/// the shared [`Atlas`](crate::atlas::Atlas), so caching a label is just caching
/// where its glyphs go — no per-label texture.
#[derive(Clone, Copy)]
struct PlacedGlyph {
    rel_x: f32,
    rel_y: f32,
    w: f32,
    h: f32,
    uv: [f32; 4],
    /// Identity of the rasterized glyph, kept so a halo can ask the atlas for a
    /// **blurred** variant of this same glyph at emission time. The layout itself is
    /// cached independently of any glow, so the blur cannot be resolved earlier.
    cache_key: CacheKey,
}

/// A shaped label's cached layout: its glyph quads + the metrics needed to place
/// the whole label. A cache hit re-emits the quads (cheap); only a **miss** shapes
/// the text and rasterizes any not-yet-seen glyphs into the atlas.
struct LabelLayout {
    glyphs: Vec<PlacedGlyph>,
    /// Ink width in physical px (`0` = empty/whitespace), for horizontal alignment.
    content_w: u32,
    /// Stable line-box metrics (physical px) for baseline-anchored centering.
    line_top: f32,
    line_height: f32,
    /// Top of the ink box (physical px) for the ink offset within the line box.
    min_y: i32,
    /// Last frame this label was used, for eviction of stale (dynamic) text.
    last_used: u64,
}

/// One placed glyph within a shaped terminal run. Horizontal position is split
/// into a cell column (snapped to the grid at emission via `col * cell_w`) plus an
/// intra-cell offset in physical px (the glyph's natural offset from its cell's
/// pen origin, i.e. side bearing + any within-cell shaping). A ligature is a
/// single glyph whose `col` is its first cell; it then renders across the cells it
/// spans without disturbing the grid.
#[derive(Clone, Copy)]
struct RunGlyph {
    /// Cell column offset (relative to the run start) this glyph snaps to.
    col: u16,
    /// Intra-cell horizontal offset (physical px) added after the cell snap.
    off_x: f32,
    /// Glyph top in physical px relative to the run's line top.
    top: f32,
    w: f32,
    h: f32,
    uv: [f32; 4],
}

/// A shaped terminal run's cached layout: per-glyph cell-snapped placement plus
/// the stable line metrics used to center within the cell box. Emission applies
/// `cell_w` and the run origin; only a cache miss re-shapes the run.
struct RunLayout {
    glyphs: Vec<RunGlyph>,
    /// Stable line-box metrics (physical px) for baseline-anchored centering.
    line_top: f32,
    line_height: f32,
    /// Last frame this run was used, for eviction of stale (dynamic) text.
    last_used: u64,
}

/// Identity of an **emitted** label: the full set of command inputs that determine
/// its final screen-space geometry — content + box + color + scale. Same key ⇒ byte-
/// identical quads, so the placed vertices are cached across frames and a static
/// label is re-appended (a `memcpy`) instead of re-placed glyph-by-glyph. This is the
/// retained-geometry layer on top of [`LabelLayout`] (which caches only *shaping*):
/// only labels whose inputs actually changed (moved/recolored/new text) get rebuilt.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct EmitKey {
    text: String,
    /// Scaled font size in `f32::to_bits()` form (exact, hashable).
    size_bits: u32,
    bold: bool,
    icon: bool,
    italic: bool,
    faux_italic: bool,
    /// Baked vertex color (`f32::to_bits()` per channel).
    color_bits: [u32; 4],
    /// Command box `(x, y, w, h)` in `f32::to_bits()` form.
    box_bits: [u32; 4],
    /// `TextAlign` discriminant (0/1/2).
    align: u8,
    centered: bool,
    line_box: bool,
    font_family: Option<String>,
    /// Scale factor in `f32::to_bits()` form.
    scale_bits: u32,
    /// True for terminal row-runs (cell-snapped emission path).
    run: bool,
    /// Cell width in `f32::to_bits()` form for terminal runs (`0` otherwise);
    /// it drives per-glyph cell snapping, so it must be part of the identity.
    cell_w_bits: u32,
    /// Hash of a terminal run's per-column colors (`0` otherwise). The colors are
    /// baked into the vertices, so a recolor (e.g. syntax highlighting changing)
    /// must miss the retained-geometry cache and rebuild.
    run_colors_hash: u64,
    /// Whether ligatures are applied (terminal runs only; `true` otherwise).
    /// Toggling it changes the shaped glyphs, so it must be part of the identity.
    ligatures: bool,
    /// Halo composite mode (`glow_alpha_scale`) in `f32::to_bits()` form, or `0`
    /// when the run has no visible halo.
    glow_mode_bits: u32,
    /// Halo colour/radius/intensity in `f32::to_bits()` form (`None` → all zero).
    /// The halo quads are baked into the cached vertices, so changing the glow must
    /// miss the retained-geometry cache and rebuild.
    glow_bits: [u32; 6],
}

/// `Option<TextGlow>` → stable bits for [`EmitKey`]. An inert glow
/// ([`TextGlow::is_visible`]) hashes identically to `None`, because both emit no
/// halo and must therefore share cached geometry rather than rebuild it.
fn glow_bits(glow: Option<TextGlow>) -> [u32; 6] {
    match glow.filter(|g| g.is_visible()) {
        Some(g) => [
            g.color[0].to_bits(),
            g.color[1].to_bits(),
            g.color[2].to_bits(),
            g.color[3].to_bits(),
            g.radius.to_bits(),
            g.intensity.to_bits(),
        ],
        None => [0; 6],
    }
}

/// A label's fully-placed glyph quads in screen space, cached across frames. The
/// vertices are absolute (logical px) with color already baked in, so emitting a
/// cached label is just appending these (no per-glyph placement, no atlas lookups).
struct Emitted {
    /// Quad vertices, 4 per glyph; indices are regenerated on append (6 per quad).
    verts: Vec<TextVertex>,
    /// Screen-space ink bounds `[x, y, w, h]` (logical px) for damage/clip culling.
    bbox: [f32; 4],
    /// Last frame this emission was used, for eviction of stale (dynamic) text.
    last_used: u64,
}

/// `TextAlign` → a stable discriminant for hashing into [`EmitKey`].
fn align_bits(align: TextAlign) -> u8 {
    match align {
        TextAlign::Start => 0,
        TextAlign::Center => 1,
        TextAlign::End => 2,
    }
}

/// FNV-1a hash of a terminal run's per-column colors, folded into the [`EmitKey`]
/// so a recolored run (same text/position) misses the retained-geometry cache.
fn hash_colors(colors: &[[f32; 4]]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for c in colors {
        for ch in c {
            h ^= ch.to_bits() as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    // A genuinely empty run hashes to a non-zero sentinel so it never collides
    // with the "not a run" `0`.
    if colors.is_empty() { 1 } else { h }
}

pub struct TextRenderer {
    /// Glyph-halo compositing mode for the current scene (see
    /// [`set_glow_alpha_scale`](TextRenderer::set_glow_alpha_scale)).
    glow_alpha_scale: f32,
    font_system: FontSystem,
    swash_cache: SwashCache,
    pipeline: wgpu::RenderPipeline,
    /// Stencil-test variant: same as `pipeline` but tests `Equal` against the
    /// rounded content-clip mask. Used when `render` is given a stencil view.
    stencil_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    scale_factor: f64,
    /// Physical framebuffer size `[w, h]` for clamping scissor rects.
    target_size: [u32; 2],
    /// Clip rect (logical px) applied to subsequently queued text; `None` = unclipped.
    current_clip: Option<[f32; 4]>,
    /// Frame-level damage region (logical px); each label is also scissored to it
    /// for damage-region redraw. `None` = full frame.
    damage: Option<[f32; 4]>,
    /// Vertices/indices already written to the persistent buffers this frame. Each
    /// pass appends at its own offset (via `queue.write_buffer`) rather than
    /// allocating a staging buffer per frame. Reset by `begin_frame`.
    frame_vtx: u32,
    frame_idx: u32,
    commands: Vec<TextCommand>,
    font_family: String,
    icon_family: String,
    /// Shared glyph atlas — one texture holding every rasterized glyph.
    atlas: crate::atlas::Atlas,
    /// The single bind group (uniform + atlas + sampler) bound for all text draws.
    atlas_bind_group: wgpu::BindGroup,
    /// Shaped-label layout cache, keyed by [`LabelKey`]. Lets static text skip
    /// per-frame shaping; glyph bitmaps are cached separately in the atlas.
    label_cache: std::collections::HashMap<LabelKey, LabelLayout>,
    /// Shaped run-layout cache for terminal row-runs, keyed by [`LabelKey`]. Each
    /// entry stores per-glyph cell-column + intra-cell offsets so the emission can
    /// snap glyphs to the grid. Separate from `label_cache` because the placement
    /// model differs (cell-snapped, not ink-box-relative).
    run_layout_cache: std::collections::HashMap<LabelKey, RunLayout>,
    /// Retained emitted-geometry cache, keyed by [`EmitKey`]. Lets an *unchanged*
    /// label skip per-frame glyph placement — its quads are re-appended verbatim.
    emit_cache: std::collections::HashMap<EmitKey, Emitted>,
    /// Monotonic frame counter (advanced by [`begin_frame`](Self::begin_frame), so it
    /// counts visual frames, not render passes) driving cache eviction.
    frame: u64,
}

/// Drop shaped layouts unused for this many frames (~a few seconds) so dynamic text
/// (counters, clocks) doesn't leak cache entries. Glyph bitmaps stay in the atlas.
const LABEL_EVICT_AFTER: u64 = 240;
/// Drop emitted geometry unused for this many frames. Position-keyed, so an animating
/// (moving) label churns entries every frame — evict promptly to keep the cache to
/// roughly the set of on-screen labels.
const EMIT_EVICT_AFTER: u64 = 4;

impl TextRenderer {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("text_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("text.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("text_uniforms"),
            contents: bytemuck::cast_slice(&[Uniforms {
                screen_size: [1.0, 1.0],
                _pad: [0.0; 2],
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("text_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("text_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("text_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("text_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<TextVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: (std::mem::size_of::<[f32; 4]>() * 2) as wgpu::BufferAddress,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Stencil-test variant: identical to `pipeline` but tests the rounded
        // content-clip mask (`Equal` to ref 1, set per pass). Depth disabled.
        let stencil_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("text_stencil_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<TextVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: (std::mem::size_of::<[f32; 4]>() * 2) as wgpu::BufferAddress,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(content_clip_stencil_state()),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Per-glyph quads (vs one quad per label) → more vertices/indices; size
        // generously. Indices are u32 (a frame can exceed 65 536 vertices).
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("text_vertex_buffer"),
            size: 4 * 1024 * 1024,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("text_index_buffer"),
            size: 1024 * 1024,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // The shared glyph atlas + the single bind group bound for all text draws.
        let atlas = crate::atlas::Atlas::new(device);
        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("text_atlas_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(atlas.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Embed the default mono font (Geist Mono) so the Grid look renders
        // without a system install. This is a default, not a lock-in:
        // `set_font_family` still overrides it from the theme.
        let mut font_system = FontSystem::new();
        font_system
            .db_mut()
            .load_font_data(heca_grid_ui::font::DEFAULT_MONO_BYTES.to_vec());
        font_system
            .db_mut()
            .load_font_data(heca_grid_ui::font::DEFAULT_MONO_BOLD_BYTES.to_vec());
        // Embedded icon font (Phosphor Duotone), registered as a second family;
        // selected per text run via `TextCommand::icon`.
        font_system
            .db_mut()
            .load_font_data(heca_grid_ui::font::ICON_FONT_BYTES.to_vec());
        font_system
            .db_mut()
            .load_font_data(font::DEFAULT_TERMINAL_BYTES.to_vec());
        font_system
            .db_mut()
            .load_font_data(font::DEFAULT_TERMINAL_BOLD_BYTES.to_vec());

        Self {
            glow_alpha_scale: 0.0,
            font_system,
            swash_cache: SwashCache::new(),
            pipeline,
            stencil_pipeline,
            vertex_buffer,
            index_buffer,
            uniform_buffer,
            scale_factor: 1.0,
            target_size: [1, 1],
            current_clip: None,
            damage: None,
            frame_vtx: 0,
            frame_idx: 0,
            commands: Vec::new(),
            font_family: heca_grid_ui::font::DEFAULT_MONO_FAMILY.to_string(),
            icon_family: heca_grid_ui::font::ICON_FONT_FAMILY.to_string(),
            atlas,
            atlas_bind_group,
            label_cache: std::collections::HashMap::new(),
            run_layout_cache: std::collections::HashMap::new(),
            emit_cache: std::collections::HashMap::new(),
            frame: 0,
        }
    }

    pub fn set_screen_size(&mut self, queue: &wgpu::Queue, width: f32, height: f32) {
        let uniforms = Uniforms {
            screen_size: [width, height],
            _pad: [0.0; 2],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    /// Select how glyph halos composite for this scene, mirroring the rect
    /// renderer's `glow_alpha_scale`: `0.0` = additive (dark backgrounds), non-zero
    /// = translucent tinted halo (light backgrounds, where *adding* light to a
    /// near-white surface does nothing).
    ///
    /// Baked into the cached geometry, so a change invalidates the emission cache.
    pub fn set_glow_alpha_scale(&mut self, scale: f32) {
        self.glow_alpha_scale = scale;
    }

    pub fn set_scale_factor(&mut self, scale: f64) {
        if (scale - self.scale_factor).abs() <= f64::EPSILON {
            return;
        }
        self.scale_factor = scale;
        // Cached glyphs/layouts were rasterized for the old scale and are now wrong;
        // clear them. Clearing the atlas also keeps it from filling up as you zoom
        // through many scales (each scale is a distinct glyph set) — which otherwise
        // latched the atlas "full" and made all new text disappear.
        self.atlas.clear();
        self.label_cache.clear();
        self.run_layout_cache.clear();
        self.emit_cache.clear();
    }

    /// Physical framebuffer size in pixels; scissor rects are clamped to it.
    pub fn set_target_size(&mut self, width: u32, height: u32) {
        self.target_size = [width.max(1), height.max(1)];
    }

    /// Set the frame-level damage region (logical px) every label is scissored to,
    /// or `None` for a full-frame render. Set once per frame before `render`.
    pub fn set_damage(&mut self, damage: Option<[f32; 4]>) {
        self.damage = damage;
    }

    /// Reset the per-frame buffer write offsets and advance the frame counter. Call
    /// once at the start of each frame, before any `render` passes — the counter
    /// drives cache eviction, so it must count visual frames, not the (multiple)
    /// render passes within a frame.
    pub fn begin_frame(&mut self) {
        self.frame_vtx = 0;
        self.frame_idx = 0;
        self.frame += 1;
    }

    /// Set the clip rect (logical px, `[x, y, w, h]`) applied to subsequently
    /// queued text, or `None` to clear it.
    pub fn set_clip(&mut self, clip: Option<[f32; 4]>) {
        self.current_clip = clip;
    }

    pub fn set_font_family(&mut self, family: &str) {
        if self.font_family == family {
            return;
        }
        self.font_family = family.to_string();
        // Caches key on text/size/weight, not family — a family swap invalidates both.
        self.label_cache.clear();
        self.run_layout_cache.clear();
        self.emit_cache.clear();
    }

    /// Measure one logical terminal cell for the given monospace family/size.
    ///
    /// This uses the same shaping stack as runtime terminal text instead of
    /// theme heuristics, so PTY grid sizing can track the real loaded font.
    pub fn measure_monospace_cell(
        &mut self,
        font_size: f32,
        font_family: &str,
    ) -> Option<(f32, f32)> {
        if !font_size.is_finite() || font_size <= 0.0 {
            return None;
        }

        let scale = self.scale_factor as f32;
        if !scale.is_finite() || scale <= 0.0 {
            return None;
        }

        let scaled_size = font_size * scale;
        let metrics = Metrics::new(scaled_size, scaled_size * 1.2);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_size(
            Some(scaled_size * 4.0),
            Some(metrics.line_height * 2.0),
        );
        let attrs = Attrs::new()
            .family(Family::Name(font_family))
            .weight(Weight::NORMAL)
            .style(Style::Normal);
        buffer.set_text("M", &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.font_system, false);

        let mut line_width = None;
        let mut line_height = metrics.line_height;
        let mut font_id = None;
        if let Some(run) = buffer.layout_runs().next() {
            line_width = Some(run.line_w.max(0.0));
            line_height = run.line_height.max(metrics.line_height);
            font_id = run.glyphs.first().map(|glyph| glyph.font_id);
        }

        if let Some(font_id) = font_id
            && let Some(font) = self.font_system.get_font(font_id, Weight::NORMAL)
        {
            if let Some(monospace_em_width) = font.monospace_em_width() {
                line_width = Some(monospace_em_width * scaled_size);
            }

            let font_metrics = font.as_swash().metrics(&[]).scale(scaled_size);
            let font_line_height =
                (font_metrics.ascent + font_metrics.descent + font_metrics.leading).max(0.0);
            if font_line_height > 0.0 {
                line_height = font_line_height;
            }
        }

        let width = line_width.unwrap_or(scaled_size * 0.6) / scale;
        let height = line_height.max(scaled_size) / scale;
        Some((width.max(1.0), height.max(1.0)))
    }

    /// Queue text with its top-left at `(x, y)` (logical px) — the simple
    /// point-positioned form used throughout the app (sidebar, chrome, …).
    pub fn queue_text(&mut self, text: &str, x: f32, y: f32, font_size: f32, color: [f32; 4]) {
        self.commands.push(TextCommand {
            text: text.to_string(),
            x,
            y,
            w: 0.0,
            h: 0.0,
            font_size,
            color,
            bold: false,
            italic: false,
            faux_italic: false,
            align: TextAlign::Start,
            centered: false,
            line_box: false,
            icon: false,
            font_family: None,
            clip: self.current_clip,
            glow: None,
            run: None,
        });
    }

    /// Queue text centered within the box `(x, y, w, h)` (logical px):
    /// horizontally per `align`, always centered vertically. Used by the grid
    /// scene renderer ([`crate::scene`]).
    #[expect(
        clippy::too_many_arguments,
        reason = "Public text-box API keeps geometry, styling, and alignment explicit for renderer call sites."
    )]
    pub fn queue_text_in_box(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        font_size: f32,
        color: [f32; 4],
        bold: bool,
        italic: bool,
        align: TextAlign,
        icon: bool,
        // Family override for this run — `None` shapes with the theme's text family. The Nerd Font
        // role arrives here (see `scene.rs`); the terminal path already used the same field.
        family: Option<&str>,
        glow: Option<TextGlow>,
    ) {
        self.commands.push(TextCommand {
            text: text.to_string(),
            x,
            y,
            w,
            h,
            font_size,
            color,
            bold,
            // Italic is the **synthesized oblique**, not `Style::Italic`. The embedded UI family
            // (Geist Mono) ships no italic face: asking cosmic-text for one would either render
            // upright or substitute a *proportional* fallback, and a proportional fallback breaks
            // the monospace advances every grid-ui measure assumes. The shear keeps the same face —
            // same glyphs, same widths, just slanted.
            italic: false,
            faux_italic: italic,
            align,
            centered: true,
            line_box: false,
            icon,
            font_family: family.map(str::to_string),
            clip: self.current_clip,
            glow,
            run: None,
        });
    }

    pub fn queue_text_in_line_box_with_style(
        &mut self,
        text: &str,
        rect: TextBox,
        font_size: f32,
        style: TextStyle<'_>,
        align: TextAlign,
        icon: bool,
    ) {
        self.commands.push(TextCommand {
            text: text.to_string(),
            x: rect.x,
            y: rect.y,
            w: rect.w,
            h: rect.h,
            font_size,
            color: style.color,
            bold: style.bold,
            italic: style.italic,
            faux_italic: style.faux_italic,
            align,
            centered: false,
            line_box: true,
            icon,
            font_family: style.font_family.map(str::to_string),
            clip: self.current_clip,
            glow: None,
            run: None,
        });
    }

    /// Queue one shaped terminal row-run: consecutive same-style cells whose text
    /// is shaped together so OpenType ligatures/contextual alternates can form.
    /// `cell_w` (logical) and `byte_cols` (per-byte cell-column offsets) let the
    /// emission snap every glyph back to its originating cell column, preserving
    /// the monospace grid even when a ligature collapses several cells into one
    /// glyph. `rect.x`/`rect.y` are the run origin; `rect.h` is the cell height
    /// used for vertical centering. Used only by the terminal text path.
    #[expect(
        clippy::too_many_arguments,
        reason = "Terminal run path threads grid metadata (cell width, per-byte columns, per-column colors, ligature flag) explicitly."
    )]
    pub fn queue_terminal_run(
        &mut self,
        text: &str,
        rect: TextBox,
        font_size: f32,
        style: TextStyle<'_>,
        cell_w: f32,
        byte_cols: &[u16],
        col_colors: &[[f32; 4]],
        ligatures: bool,
    ) {
        self.commands.push(TextCommand {
            text: text.to_string(),
            x: rect.x,
            y: rect.y,
            w: rect.w,
            h: rect.h,
            font_size,
            color: style.color,
            bold: style.bold,
            italic: style.italic,
            faux_italic: style.faux_italic,
            align: TextAlign::Start,
            centered: false,
            line_box: true,
            icon: false,
            font_family: style.font_family.map(str::to_string),
            clip: self.current_clip,
            // Never: cell glyphs are the hottest path in the app (thousands per
            // frame) and a halo multiplies their vertex count by the ring taps.
            glow: None,
            run: Some(TerminalRun {
                cell_w,
                byte_cols: byte_cols.to_vec(),
                col_colors: col_colors.to_vec(),
                ligatures,
            }),
        });
    }

    /// Render all queued text. Returns (vertices, indices, labels) for drawing.
    /// Shape every queued label and emit per-glyph quads, pulling glyph bitmaps from
    /// the shared atlas. A label's layout (its placed glyphs) is cached so static
    /// text isn't re-shaped; the glyphs themselves are cached in the atlas, so *new*
    /// text only rasterizes glyphs it hasn't seen — no per-label texture.
    fn build_labels(&mut self, queue: &wgpu::Queue) -> (Vec<TextVertex>, Vec<u32>, Vec<DrawRange>) {
        let commands = std::mem::take(&mut self.commands);
        if commands.is_empty() {
            return (Vec::new(), Vec::new(), Vec::new());
        }

        let scale = self.scale_factor as f32;
        let mut vertices: Vec<TextVertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        let mut draws: Vec<DrawRange> = Vec::new();
        let frame = self.frame;

        for cmd in &commands {
            let scaled_size = cmd.font_size * scale;
            let emit_key = EmitKey {
                glow_bits: glow_bits(cmd.glow),
                // The halo's colour AND its composite mode are baked into the
                // cached vertices, so switching theme brightness must rebuild.
                glow_mode_bits: cmd
                    .glow
                    .filter(|g| g.is_visible())
                    .map_or(0, |_| self.glow_alpha_scale.to_bits()),
                text: cmd.text.clone(),
                size_bits: scaled_size.to_bits(),
                bold: cmd.bold,
                icon: cmd.icon,
                italic: cmd.italic,
                faux_italic: cmd.faux_italic,
                color_bits: cmd.color.map(f32::to_bits),
                box_bits: [cmd.x, cmd.y, cmd.w, cmd.h].map(f32::to_bits),
                align: align_bits(cmd.align),
                centered: cmd.centered,
                line_box: cmd.line_box,
                font_family: cmd.font_family.clone(),
                scale_bits: scale.to_bits(),
                run: cmd.run.is_some(),
                cell_w_bits: cmd.run.as_ref().map_or(0, |r| r.cell_w.to_bits()),
                run_colors_hash: cmd.run.as_ref().map_or(0, |r| hash_colors(&r.col_colors)),
                ligatures: cmd.run.as_ref().is_none_or(|r| r.ligatures),
            };

            // Retained-geometry miss → place every glyph (shaping itself is cached in
            // `label_cache`, glyph bitmaps in the atlas) and stash the final quads. A
            // hit skips straight to the append below — no per-glyph work.
            if !self.emit_cache.contains_key(&emit_key) {
                let emitted = self.build_emission(queue, cmd, scaled_size, scale);
                self.emit_cache.insert(emit_key.clone(), emitted);
            }

            let emitted = self.emit_cache.get_mut(&emit_key).expect("just inserted");
            emitted.last_used = frame;
            if emitted.verts.is_empty() {
                continue; // nothing to draw (whitespace/empty)
            }

            // Cull: a label entirely outside this frame's damage∩clip region needs no
            // geometry at all (the per-draw scissor in `render` would clip it away
            // anyway, but emitting + uploading it is wasted work — the whole point on a
            // small-damage frame like a caret blink). Skip only when *provably* outside.
            if let Some(c) = combine_clip(self.damage, cmd.clip) {
                let i = intersect(c, emitted.bbox);
                if i[2] <= 0.0 || i[3] <= 0.0 {
                    continue;
                }
            }

            let first_index = indices.len() as u32;
            let mut base = vertices.len() as u32;
            let quads = emitted.verts.chunks_exact(4);
            debug_assert!(
                quads.remainder().is_empty(),
                "emitted terminal/text geometry must contain 4 vertices per glyph quad"
            );
            for quad in quads {
                vertices.extend_from_slice(quad);
                indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
                base += 4;
            }
            draws.push(DrawRange {
                clip: cmd.clip,
                first_index,
                index_count: indices.len() as u32 - first_index,
            });
        }

        // Evict cache entries unused for a while so dynamic/animating text doesn't grow
        // either cache unbounded. Atlas glyphs are kept (a bounded set).
        self.emit_cache
            .retain(|_, e| frame.saturating_sub(e.last_used) <= EMIT_EVICT_AFTER);
        self.label_cache
            .retain(|_, l| frame.saturating_sub(l.last_used) <= LABEL_EVICT_AFTER);
        self.run_layout_cache
            .retain(|_, l| frame.saturating_sub(l.last_used) <= LABEL_EVICT_AFTER);

        (vertices, indices, draws)
    }

    /// Place one label's glyphs into final screen-space quads (logical px, color
    /// baked). Shaping is reused from `label_cache` on a hit; a miss shapes the text
    /// and rasterizes any not-yet-seen glyphs into the atlas. Returns the [`Emitted`]
    /// quads + their ink bbox for the retained-geometry cache.
    fn build_emission(
        &mut self,
        queue: &wgpu::Queue,
        cmd: &TextCommand,
        scaled_size: f32,
        scale: f32,
    ) -> Emitted {
        if cmd.run.is_some() {
            return self.build_run_emission(queue, cmd, scaled_size, scale);
        }
        let key = LabelKey {
            text: cmd.text.clone(),
            size_bits: scaled_size.to_bits(),
            bold: cmd.bold,
            icon: cmd.icon,
            italic: cmd.italic,
            font_family: cmd.font_family.clone(),
            ligatures: true,
        };

        // Shape on a `label_cache` miss; rasterize each glyph into the atlas on its
        // first sighting.
        if !self.label_cache.contains_key(&key) {
            let metrics = Metrics::new(scaled_size, scaled_size * 1.2);
            let mut buffer = Buffer::new(&mut self.font_system, metrics);
            buffer.set_size(Some(10000.0), Some(10000.0));
            let weight = if cmd.bold {
                Weight::BOLD
            } else {
                Weight::NORMAL
            };
            let family = if cmd.icon {
                self.icon_family.as_str()
            } else {
                cmd.font_family.as_deref().unwrap_or(&self.font_family)
            };
            let attrs = Attrs::new()
                .family(Family::Name(family))
                .weight(weight)
                .style(if cmd.italic {
                    Style::Italic
                } else {
                    Style::Normal
                });
            buffer.set_text(&cmd.text, &attrs, Shaping::Advanced, None);
            buffer.shape_until_scroll(&mut self.font_system, false);

            // Stable, glyph-independent line metrics for vertical centering so the
            // baseline doesn't jump with ascenders/descenders.
            let mut min_x = i32::MAX;
            let mut min_y = i32::MAX;
            let mut max_x = i32::MIN;
            let mut line_top = 0.0f32;
            let mut line_height = 0.0f32;
            // (glyph left, glyph top in physical px, atlas entry).
            let mut placed: Vec<(i32, i32, crate::atlas::AtlasGlyph, CacheKey)> = Vec::new();
            for run in buffer.layout_runs() {
                line_top = run.line_top;
                line_height = run.line_height;
                for glyph in run.glyphs {
                    let physical = glyph.physical((0.0, run.line_y), 1.0);
                    let Some(ag) = self.atlas.glyph(
                        queue,
                        &mut self.font_system,
                        &mut self.swash_cache,
                        physical.cache_key,
                    ) else {
                        continue; // whitespace / no bitmap
                    };
                    let left = physical.x + ag.left;
                    let top = physical.y - ag.top;
                    min_x = min_x.min(left);
                    min_y = min_y.min(top);
                    max_x = max_x.max(left + ag.width as i32);
                    placed.push((left, top, ag, physical.cache_key));
                }
            }

            let layout = if placed.is_empty() {
                // Empty/whitespace — cache an empty layout so we don't re-shape it.
                LabelLayout {
                    glyphs: Vec::new(),
                    content_w: 0,
                    line_top,
                    line_height,
                    min_y: 0,
                    last_used: self.frame,
                }
            } else {
                let glyphs = placed
                    .into_iter()
                    .map(|(l, t, ag, cache_key)| PlacedGlyph {
                        rel_x: (l - min_x) as f32,
                        rel_y: (t - min_y) as f32,
                        w: ag.width as f32,
                        h: ag.height as f32,
                        uv: ag.uv,
                        cache_key,
                    })
                    .collect();
                LabelLayout {
                    glyphs,
                    content_w: (max_x - min_x) as u32,
                    line_top,
                    line_height,
                    min_y,
                    last_used: self.frame,
                }
            };
            self.label_cache.insert(key.clone(), layout);
        }

        let layout = self.label_cache.get_mut(&key).expect("just inserted");
        layout.last_used = self.frame;
        if layout.glyphs.is_empty() {
            return Emitted {
                verts: Vec::new(),
                bbox: [0.0; 4],
                last_used: self.frame,
            };
        }

        // Centering: place the label ink box's top-left — horizontally per `align` on
        // the ink width, vertically on the stable line box — then offset each glyph by
        // its (cached) position within the ink box.
        let screen_w = layout.content_w as f32 / scale;
        let (screen_x, screen_y) = if cmd.centered || cmd.line_box {
            let x = cmd.x
                + match cmd.align {
                    TextAlign::Start => 0.0,
                    TextAlign::Center => (cmd.w - screen_w) * 0.5,
                    TextAlign::End => cmd.w - screen_w,
                };
            let line_box_top = cmd.y + (cmd.h - layout.line_height / scale) * 0.5;
            let ink_offset = (layout.min_y as f32 - layout.line_top) / scale;
            (x, line_box_top + ink_offset)
        } else {
            (cmd.x, cmd.y)
        };

        let faux_italic_skew = if cmd.faux_italic {
            (layout.line_height / scale * 0.14).max(1.0)
        } else {
            0.0
        };
        // One textured quad in screen space. Takes explicit geometry because the
        // sharp glyph and its halo sample different atlas entries at different sizes.
        let push_quad = |verts: &mut Vec<TextVertex>,
                         rel_x: f32,
                         rel_y: f32,
                         w: f32,
                         h: f32,
                         uv: [f32; 4],
                         color: [f32; 4],
                         additive: f32| {
            let gx = screen_x + rel_x / scale;
            let gy = screen_y + rel_y / scale;
            let gw = w / scale;
            let gh = h / scale;
            let [u0, v0, u1, v1] = uv;
            let top_x = gx + faux_italic_skew * 0.5;
            let bottom_x = gx - faux_italic_skew * 0.5;
            for (position, texcoord) in [
                ([top_x, gy], [u0, v0]),
                ([top_x + gw, gy], [u1, v0]),
                ([bottom_x + gw, gy + gh], [u1, v1]),
                ([bottom_x, gy + gh], [u0, v1]),
            ] {
                verts.push(TextVertex {
                    position,
                    texcoord,
                    color,
                    additive,
                });
            }
        };

        let halo = cmd.glow.filter(|g| g.is_visible());
        let mut verts: Vec<TextVertex> =
            Vec::with_capacity(layout.glyphs.len() * 4 * (1 + usize::from(halo.is_some())));

        // Halo first, so the sharp glyphs land on top of it. ONE quad per glyph,
        // sampling a pre-blurred copy of that glyph's coverage mask — a continuous
        // falloff, rather than a stack of offset copies whose shells read as spokes.
        let mut halo_reach = 0.0f32;
        if let Some(g) = halo {
            // Light backgrounds take a translucent tinted halo; dark ones take added
            // light. Adding light to a near-white surface does nothing, which is why
            // the rect renderer makes the same switch (`grid.wgsl`'s alpha path).
            let tinted = self.glow_alpha_scale > 0.0;
            let additive = if tinted { 0.0 } else { 1.0 };
            let base = g.color[3] * g.intensity;
            // Because the blurred mask is renormalized to full peak, `base` IS the
            // halo's peak alpha — the same quantity the rect shader's glow peaks at,
            // so the two are directly comparable rather than eyeballed.
            let alpha = if tinted {
                // Mirror the rect shader's light-background curve (`grid.wgsl`):
                // the square root lifts a faint intensity into a visible tint, which
                // a linear scale cannot do against a near-white surface.
                base.sqrt() * self.glow_alpha_scale * HALO_LIGHT_GAIN
            } else {
                base * HALO_DARK_GAIN
            }
            .clamp(0.0, 1.0);
            let color = [g.color[0], g.color[1], g.color[2], alpha];
            // Sigma is in physical px, like everything the atlas rasterizes.
            let sigma = g.radius * HALO_SIGMA_FRAC * scale;
            halo_reach = g.radius;

            // Collected first: the atlas needs `&mut self` while `push_quad` borrows
            // the frame's placement values.
            let blurred: Vec<(usize, crate::atlas::AtlasGlyph)> = layout
                .glyphs
                .iter()
                .enumerate()
                .filter_map(|(i, glyph)| {
                    let entry = self.atlas.glyph_blurred(
                        queue,
                        &mut self.font_system,
                        &mut self.swash_cache,
                        glyph.cache_key,
                        sigma,
                    )?;
                    Some((i, entry))
                })
                .collect();

            for (i, entry) in blurred {
                let glyph = &layout.glyphs[i];
                // The blurred bitmap is the glyph's, grown by the blur's padding on
                // every side; recover that padding to place it concentrically.
                let pad = (entry.width as f32 - glyph.w) * 0.5;
                push_quad(
                    &mut verts,
                    glyph.rel_x - pad,
                    glyph.rel_y - pad,
                    entry.width as f32,
                    entry.height as f32,
                    entry.uv,
                    color,
                    additive,
                );
            }
        }

        let (mut bx0, mut by0) = (f32::MAX, f32::MAX);
        let (mut bx1, mut by1) = (f32::MIN, f32::MIN);
        for g in &layout.glyphs {
            let gx = screen_x + g.rel_x / scale;
            let gy = screen_y + g.rel_y / scale;
            let gw = g.w / scale;
            let gh = g.h / scale;
            let top_x = gx + faux_italic_skew * 0.5;
            let bottom_x = gx - faux_italic_skew * 0.5;
            push_quad(
                &mut verts,
                g.rel_x,
                g.rel_y,
                g.w,
                g.h,
                g.uv,
                cmd.color,
                0.0,
            );
            bx0 = bx0.min(bottom_x);
            by0 = by0.min(gy);
            bx1 = bx1.max(top_x + gw);
            by1 = by1.max(gy + gh);
        }

        // The ink box must contain the halo too, or damage/clip culling would
        // scissor away the very light the halo adds outside the glyph box.
        let reach = halo_reach;
        Emitted {
            verts,
            bbox: [
                bx0 - reach,
                by0 - reach,
                (bx1 - bx0) + reach * 2.0,
                (by1 - by0) + reach * 2.0,
            ],
            last_used: self.frame,
        }
    }

    /// Emit one terminal row-run: shape the whole run text together (so OpenType
    /// ligatures/contextual alternates can form across cells), then snap every
    /// glyph back to its originating cell column so the monospace grid is exact.
    /// Shaping is reused from `run_layout_cache` on a hit.
    fn build_run_emission(
        &mut self,
        queue: &wgpu::Queue,
        cmd: &TextCommand,
        scaled_size: f32,
        scale: f32,
    ) -> Emitted {
        let run = cmd.run.as_ref().expect("build_run_emission requires run info");
        let key = LabelKey {
            text: cmd.text.clone(),
            size_bits: scaled_size.to_bits(),
            bold: cmd.bold,
            icon: false,
            italic: cmd.italic,
            font_family: cmd.font_family.clone(),
            ligatures: run.ligatures,
        };

        if !self.run_layout_cache.contains_key(&key) {
            let metrics = Metrics::new(scaled_size, scaled_size * 1.2);
            let mut buffer = Buffer::new(&mut self.font_system, metrics);
            buffer.set_size(Some(10000.0), Some(10000.0));
            let weight = if cmd.bold {
                Weight::BOLD
            } else {
                Weight::NORMAL
            };
            let family = cmd.font_family.as_deref().unwrap_or(&self.font_family);
            let mut attrs = Attrs::new()
                .family(Family::Name(family))
                .weight(weight)
                .style(if cmd.italic {
                    Style::Italic
                } else {
                    Style::Normal
                });
            // `Shaping::Advanced` runs the full HarfBuzz-style shaper; with the run's
            // whole text visible, `calt`/`liga`/`clig` (on by default in coding fonts)
            // collapse e.g. `->` into a ligature. When the user disables ligatures,
            // turn those features off so each character shapes standalone.
            if !run.ligatures {
                let mut features = FontFeatures::new();
                features
                    .disable(FeatureTag::CONTEXTUAL_ALTERNATES)
                    .disable(FeatureTag::STANDARD_LIGATURES)
                    .disable(FeatureTag::CONTEXTUAL_LIGATURES);
                attrs = attrs.font_features(features);
            }
            buffer.set_text(&cmd.text, &attrs, Shaping::Advanced, None);
            buffer.shape_until_scroll(&mut self.font_system, false);

            // First pass: collect raw placements (cell column, pen x, bearing, top).
            // `glyph.start` is a byte offset into the run text; `byte_cols` maps it to
            // the cell column the glyph anchors to (a ligature anchors to its first
            // cell). We keep cosmic's natural pen x only to recover the glyph's offset
            // *within* its cell — the absolute pen is discarded for the grid snap.
            let last_col = run.byte_cols.last().copied().unwrap_or(0);
            let mut line_top = 0.0f32;
            let mut line_height = 0.0f32;
            // (col, pen_x, left, top, atlas glyph)
            let mut raw: Vec<(u16, i32, i32, i32, crate::atlas::AtlasGlyph)> = Vec::new();
            for layout_run in buffer.layout_runs() {
                line_top = layout_run.line_top;
                line_height = layout_run.line_height;
                for glyph in layout_run.glyphs {
                    let physical = glyph.physical((0.0, layout_run.line_y), 1.0);
                    let Some(ag) = self.atlas.glyph(
                        queue,
                        &mut self.font_system,
                        &mut self.swash_cache,
                        physical.cache_key,
                    ) else {
                        continue; // whitespace / no bitmap
                    };
                    let col = run
                        .byte_cols
                        .get(glyph.start)
                        .copied()
                        .unwrap_or(last_col);
                    let top = physical.y - ag.top;
                    raw.push((col, physical.x, ag.left, top, ag));
                }
            }

            // Per-cell anchor pen: the smallest pen x among glyphs in that cell, so a
            // glyph's intra-cell offset is `pen_x - anchor + bearing`. For the common
            // one-glyph-per-cell case this is just the side bearing.
            let mut anchor: std::collections::HashMap<u16, i32> = std::collections::HashMap::new();
            for &(col, pen_x, _, _, _) in &raw {
                anchor
                    .entry(col)
                    .and_modify(|a| *a = (*a).min(pen_x))
                    .or_insert(pen_x);
            }

            let glyphs = raw
                .into_iter()
                .map(|(col, pen_x, left, top, ag)| {
                    let off_x = (pen_x - anchor.get(&col).copied().unwrap_or(pen_x) + left) as f32;
                    RunGlyph {
                        col,
                        off_x,
                        top: top as f32,
                        w: ag.width as f32,
                        h: ag.height as f32,
                        uv: ag.uv,
                    }
                })
                .collect();

            self.run_layout_cache.insert(
                key.clone(),
                RunLayout {
                    glyphs,
                    line_top,
                    line_height,
                    last_used: self.frame,
                },
            );
        }

        let layout = self.run_layout_cache.get_mut(&key).expect("just inserted");
        layout.last_used = self.frame;
        if layout.glyphs.is_empty() {
            return Emitted {
                verts: Vec::new(),
                bbox: [0.0; 4],
                last_used: self.frame,
            };
        }

        // Vertical: center the stable line box within the cell height, then place each
        // glyph by its top relative to the line top (matches the generic line-box path,
        // so baselines don't jump with ascenders/descenders).
        let line_box_top = cmd.y + (cmd.h - layout.line_height / scale) * 0.5;
        let cell_w = run.cell_w;
        let faux_italic_skew = if cmd.faux_italic {
            (layout.line_height / scale * 0.14).max(1.0)
        } else {
            0.0
        };

        let mut verts: Vec<TextVertex> = Vec::with_capacity(layout.glyphs.len() * 4);
        let (mut bx0, mut by0) = (f32::MAX, f32::MAX);
        let (mut bx1, mut by1) = (f32::MIN, f32::MIN);
        for g in &layout.glyphs {
            // Grid snap: cell column → exact x, plus the glyph's intra-cell offset.
            let gx = cmd.x + g.col as f32 * cell_w + g.off_x / scale;
            let gy = line_box_top + (g.top - layout.line_top) / scale;
            let gw = g.w / scale;
            let gh = g.h / scale;
            let [u0, v0, u1, v1] = g.uv;
            // Color each glyph by its own cell, so a ligature crossing a color
            // boundary keeps each half tinted as its source cell.
            let color = run
                .col_colors
                .get(g.col as usize)
                .copied()
                .unwrap_or(cmd.color);
            let top_x = gx + faux_italic_skew * 0.5;
            let bottom_x = gx - faux_italic_skew * 0.5;
            verts.push(TextVertex {
                position: [top_x, gy],
                texcoord: [u0, v0],
                color,
                additive: 0.0,
            });
            verts.push(TextVertex {
                position: [top_x + gw, gy],
                texcoord: [u1, v0],
                color,
                additive: 0.0,
            });
            verts.push(TextVertex {
                position: [bottom_x + gw, gy + gh],
                texcoord: [u1, v1],
                color,
                additive: 0.0,
            });
            verts.push(TextVertex {
                position: [bottom_x, gy + gh],
                texcoord: [u0, v1],
                color,
                additive: 0.0,
            });
            bx0 = bx0.min(bottom_x);
            by0 = by0.min(gy);
            bx1 = bx1.max(top_x + gw);
            by1 = by1.max(gy + gh);
        }
        Emitted {
            verts,
            bbox: [bx0, by0, bx1 - bx0, by1 - by0],
            last_used: self.frame,
        }
    }

    pub fn render(
        &mut self,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        stencil: Option<&wgpu::TextureView>,
    ) {
        if self.commands.is_empty() {
            return;
        }

        let (vertices, indices, draws) = self.build_labels(queue);

        if vertices.is_empty() {
            self.commands.clear();
            return;
        }

        let vertex_data: &[u8] = bytemuck::cast_slice(&vertices);
        let index_data: &[u8] = bytemuck::cast_slice(&indices);

        // Append this pass's geometry to the persistent buffers at the running frame
        // offset (no per-frame staging allocation). Distinct offsets per pass means a
        // later pass (an overlay) doesn't clobber an earlier one (the base text).
        let v_off = self.frame_vtx as u64 * std::mem::size_of::<TextVertex>() as u64;
        let i_off = self.frame_idx as u64 * std::mem::size_of::<u32>() as u64;
        queue.write_buffer(&self.vertex_buffer, v_off, vertex_data);
        queue.write_buffer(&self.index_buffer, i_off, index_data);

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("text_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: stencil.as_ref().map(|view| {
                wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: None,
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                }
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        rpass.set_pipeline(if stencil.is_some() {
            &self.stencil_pipeline
        } else {
            &self.pipeline
        });
        if stencil.is_some() {
            rpass.set_stencil_reference(1);
        }
        // One bind group for all text now: the shared glyph atlas.
        rpass.set_bind_group(0, &self.atlas_bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

        // One draw per label (so each can be scissored to its clip), all sampling the
        // same atlas. Indices/vertices live at this frame's running offset, so draws
        // reference them via `first_index` (idx_base + range) + `base_vertex`.
        let scale = self.scale_factor as f32;
        let idx_base = self.frame_idx;
        let base_vertex = self.frame_vtx as i32;
        for d in &draws {
            match combine_clip(self.damage, d.clip) {
                None => rpass.set_scissor_rect(0, 0, self.target_size[0], self.target_size[1]),
                Some(c) => {
                    let (x, y, w, h) = scissor_px(c, scale, self.target_size);
                    if w == 0 || h == 0 {
                        continue; // fully outside the damage/clip — nothing visible
                    }
                    rpass.set_scissor_rect(x, y, w, h);
                }
            }
            let start = idx_base + d.first_index;
            rpass.draw_indexed(start..start + d.index_count, base_vertex, 0..1);
        }
        drop(rpass);

        self.frame_vtx += vertices.len() as u32;
        self.frame_idx += indices.len() as u32;
        self.commands.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_key_distinguishes_color_and_position() {
        let base = EmitKey {
            text: "hi".into(),
            size_bits: 14.0f32.to_bits(),
            bold: false,
            icon: false,
            italic: false,
            faux_italic: false,
            color_bits: [1.0, 1.0, 1.0, 1.0].map(f32::to_bits),
            box_bits: [10.0, 20.0, 0.0, 0.0].map(f32::to_bits),
            align: align_bits(TextAlign::Start),
            centered: false,
            line_box: false,
            font_family: None,
            scale_bits: 2.0f32.to_bits(),
            run: false,
            cell_w_bits: 0,
            run_colors_hash: 0,
            ligatures: true,
            glow_bits: glow_bits(None),
            glow_mode_bits: 0,
        };
        assert_eq!(base, base.clone(), "identical inputs ⇒ a cache hit");

        // Color is baked into the vertices, so a recolor must miss (rebuild).
        let mut recolored = base.clone();
        recolored.color_bits = [1.0, 0.0, 0.0, 1.0].map(f32::to_bits);
        assert_ne!(base, recolored, "a color change is a different emission");

        // Position is baked into the vertices, so a move must miss (rebuild).
        let mut moved = base.clone();
        moved.box_bits = [11.0, 20.0, 0.0, 0.0].map(f32::to_bits);
        assert_ne!(base, moved, "a move is a different emission");
    }

    /// The halo quads are baked into the cached vertices, so changing the glow must
    /// miss the retained-geometry cache — otherwise a glyph would keep the halo it
    /// had when it was first emitted.
    #[test]
    fn emit_key_distinguishes_the_glow() {
        let flat = glow_bits(None);
        let lit = glow_bits(Some(TextGlow {
            color: [0.0, 1.0, 1.0, 1.0],
            radius: 6.0,
            intensity: 0.5,
        }));
        assert_ne!(flat, lit);
    }

    /// A fully transparent glow emits no halo, so it must hash like no glow at all —
    /// otherwise identical output would rebuild every frame.
    #[test]
    fn emit_key_treats_a_transparent_glow_as_no_glow() {
        let transparent = glow_bits(Some(TextGlow {
            color: [0.0, 1.0, 1.0, 0.0],
            radius: 0.0,
            intensity: 0.0,
        }));
        assert_eq!(glow_bits(None), transparent);
    }

    #[test]
    fn align_bits_are_distinct() {
        let mut v = vec![
            align_bits(TextAlign::Start),
            align_bits(TextAlign::Center),
            align_bits(TextAlign::End),
        ];
        v.sort_unstable();
        v.dedup();
        assert_eq!(
            v.len(),
            3,
            "each alignment hashes to a distinct discriminant"
        );
    }
}
