use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight};
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

pub struct TextRenderer {
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
            &mut self.font_system,
            Some(scaled_size * 4.0),
            Some(metrics.line_height * 2.0),
        );
        let attrs = Attrs::new()
            .family(Family::Name(font_family))
            .weight(Weight::NORMAL)
            .style(Style::Normal);
        buffer.set_text(&mut self.font_system, "M", &attrs, Shaping::Advanced);
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
            && let Some(font) = self.font_system.get_font(font_id)
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
        });
    }

    /// Queue text centered within the box `(x, y, w, h)` (logical px):
    /// horizontally per `align`, always centered vertically. Used by the grid
    /// scene renderer ([`crate::scene`]).
    #[expect(clippy::too_many_arguments, reason = "Public text-box API keeps geometry, styling, and alignment explicit for renderer call sites.")]
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
        align: TextAlign,
        icon: bool,
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
            italic: false,
            faux_italic: false,
            align,
            centered: true,
            line_box: false,
            icon,
            font_family: None,
            clip: self.current_clip,
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
        let key = LabelKey {
            text: cmd.text.clone(),
            size_bits: scaled_size.to_bits(),
            bold: cmd.bold,
            icon: cmd.icon,
            italic: cmd.italic,
            font_family: cmd.font_family.clone(),
        };

        // Shape on a `label_cache` miss; rasterize each glyph into the atlas on its
        // first sighting.
        if !self.label_cache.contains_key(&key) {
            let metrics = Metrics::new(scaled_size, scaled_size * 1.2);
            let mut buffer = Buffer::new(&mut self.font_system, metrics);
            buffer.set_size(&mut self.font_system, Some(10000.0), Some(10000.0));
            let weight = if cmd.bold { Weight::BOLD } else { Weight::NORMAL };
            let family = if cmd.icon {
                self.icon_family.as_str()
            } else {
                cmd.font_family.as_deref().unwrap_or(&self.font_family)
            };
            let attrs = Attrs::new()
                .family(Family::Name(family))
                .weight(weight)
                .style(if cmd.italic { Style::Italic } else { Style::Normal });
            buffer.set_text(&mut self.font_system, &cmd.text, &attrs, Shaping::Advanced);
            buffer.shape_until_scroll(&mut self.font_system, false);

            // Stable, glyph-independent line metrics for vertical centering so the
            // baseline doesn't jump with ascenders/descenders.
            let mut min_x = i32::MAX;
            let mut min_y = i32::MAX;
            let mut max_x = i32::MIN;
            let mut line_top = 0.0f32;
            let mut line_height = 0.0f32;
            // (glyph left, glyph top in physical px, atlas entry).
            let mut placed: Vec<(i32, i32, crate::atlas::AtlasGlyph)> = Vec::new();
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
                    placed.push((left, top, ag));
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
                    .map(|(l, t, ag)| PlacedGlyph {
                        rel_x: (l - min_x) as f32,
                        rel_y: (t - min_y) as f32,
                        w: ag.width as f32,
                        h: ag.height as f32,
                        uv: ag.uv,
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
        let mut verts: Vec<TextVertex> = Vec::with_capacity(layout.glyphs.len() * 4);
        let (mut bx0, mut by0) = (f32::MAX, f32::MAX);
        let (mut bx1, mut by1) = (f32::MIN, f32::MIN);
        for g in &layout.glyphs {
            let gx = screen_x + g.rel_x / scale;
            let gy = screen_y + g.rel_y / scale;
            let gw = g.w / scale;
            let gh = g.h / scale;
            let [u0, v0, u1, v1] = g.uv;
            let top_x = gx + faux_italic_skew * 0.5;
            let bottom_x = gx - faux_italic_skew * 0.5;
            verts.push(TextVertex { position: [top_x, gy], texcoord: [u0, v0], color: cmd.color });
            verts.push(TextVertex { position: [top_x + gw, gy], texcoord: [u1, v0], color: cmd.color });
            verts.push(TextVertex { position: [bottom_x + gw, gy + gh], texcoord: [u1, v1], color: cmd.color });
            verts.push(TextVertex { position: [bottom_x, gy + gh], texcoord: [u0, v1], color: cmd.color });
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

    #[test]
    fn align_bits_are_distinct() {
        let mut v = vec![
            align_bits(TextAlign::Start),
            align_bits(TextAlign::Center),
            align_bits(TextAlign::End),
        ];
        v.sort_unstable();
        v.dedup();
        assert_eq!(v.len(), 3, "each alignment hashes to a distinct discriminant");
    }
}
