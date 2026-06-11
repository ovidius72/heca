use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache, Weight};
use heca_grid_ui::scene::TextAlign;
use wgpu::util::DeviceExt;

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
    align: TextAlign,
    /// Center within `(w, h)` (grid scene), or place at `(x, y)` (app labels).
    centered: bool,
    /// Shape with the embedded icon font instead of the text family.
    icon: bool,
    /// Clip rect (logical px) this label is scissored to, if any.
    clip: Option<[f32; 4]>,
}

/// A queued label's cache key plus the clip rect (logical px) it's scissored to.
type LabelDraw = (LabelKey, Option<[f32; 4]>);

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

/// A GPU-ready text label: texture + quad.
/// Identity of a rasterized label: same text + size + weight + family produces
/// the same alpha mask, so it's cached and reused across frames. Color is **not**
/// part of the key — the mask is tinted per-draw in the shader (vertex color).
#[derive(Clone, PartialEq, Eq, Hash)]
struct LabelKey {
    text: String,
    /// Scaled font size in `f32::to_bits()` form (exact, hashable).
    size_bits: u32,
    bold: bool,
    icon: bool,
}

/// A rasterized label cached on the GPU: its alpha-mask texture + bind group plus
/// the measured geometry needed to place its quad. Re-emitting the quad each frame
/// is cheap; only a cache **miss** re-shapes + re-rasterizes + uploads.
struct CachedLabel {
    /// Kept alive so the bind group's texture view stays valid.
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    /// Ink size in physical px (`0` width = empty/whitespace → nothing to draw).
    content_w: u32,
    content_h: u32,
    /// Stable line-box metrics (physical px) for baseline-anchored centering.
    line_top: f32,
    line_height: f32,
    /// Top of the ink box (physical px) for the ink offset within the line box.
    min_y: i32,
    /// Last frame this label was used, for eviction of stale (dynamic) text.
    last_used: u64,
}

pub struct TextRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
    scale_factor: f64,
    /// Physical framebuffer size `[w, h]` for clamping scissor rects.
    target_size: [u32; 2],
    /// Clip rect (logical px) applied to subsequently queued text; `None` = unclipped.
    current_clip: Option<[f32; 4]>,
    commands: Vec<TextCommand>,
    _atlas_size: (u32, u32),
    font_family: String,
    icon_family: String,
    /// Rasterized-label cache, keyed by [`LabelKey`]. Lets static text skip
    /// per-frame shaping + rasterization + texture/bind-group creation.
    label_cache: std::collections::HashMap<LabelKey, CachedLabel>,
    /// Monotonic build counter driving cache eviction.
    frame: u64,
}

/// Drop labels unused for this many builds (~a few seconds) so dynamic text
/// (counters, clocks) doesn't leak GPU textures.
const LABEL_EVICT_AFTER: u64 = 240;

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
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("text_vertex_buffer"),
            size: 1024 * 1024,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("text_index_buffer"),
            size: 512 * 1024,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
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

        Self {
            font_system,
            swash_cache: SwashCache::new(),
            pipeline,
            vertex_buffer,
            index_buffer,
            bind_group_layout,
            uniform_buffer,
            sampler,
            scale_factor: 1.0,
            target_size: [1, 1],
            current_clip: None,
            commands: Vec::new(),
            _atlas_size: (0, 0),
            font_family: heca_grid_ui::font::DEFAULT_MONO_FAMILY.to_string(),
            icon_family: heca_grid_ui::font::ICON_FONT_FAMILY.to_string(),
            label_cache: std::collections::HashMap::new(),
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
        self.scale_factor = scale;
    }

    /// Physical framebuffer size in pixels; scissor rects are clamped to it.
    pub fn set_target_size(&mut self, width: u32, height: u32) {
        self.target_size = [width.max(1), height.max(1)];
    }

    /// Set the clip rect (logical px, `[x, y, w, h]`) applied to subsequently
    /// queued text, or `None` to clear it.
    pub fn set_clip(&mut self, clip: Option<[f32; 4]>) {
        self.current_clip = clip;
    }

    pub fn set_font_family(&mut self, family: &str) {
        self.font_family = family.to_string();
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
            align: TextAlign::Start,
            centered: false,
            icon: false,
            clip: self.current_clip,
        });
    }

    /// Queue text centered within the box `(x, y, w, h)` (logical px):
    /// horizontally per `align`, always centered vertically. Used by the grid
    /// scene renderer ([`crate::scene`]).
    #[allow(clippy::too_many_arguments)]
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
            align,
            centered: true,
            icon,
            clip: self.current_clip,
        });
    }

    /// Render all queued text. Returns (vertices, indices, labels) for drawing.
    fn build_labels(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> (Vec<TextVertex>, Vec<u16>, Vec<LabelDraw>) {
        let commands = std::mem::take(&mut self.commands);
        if commands.is_empty() {
            return (Vec::new(), Vec::new(), Vec::new());
        }
        self.frame += 1;

        let scale = self.scale_factor as f32;
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut keys: Vec<LabelDraw> = Vec::new();
        let mut base = 0u16;

        for cmd in &commands {
            let scaled_size = cmd.font_size * scale;
            let key = LabelKey {
                text: cmd.text.clone(),
                size_bits: scaled_size.to_bits(),
                bold: cmd.bold,
                icon: cmd.icon,
            };

            // Cache miss → shape + rasterize + upload (the expensive path). On a
            // cache hit we skip straight to re-emitting the quad below, so static
            // text costs nothing per frame beyond a few vertices.
            if !self.label_cache.contains_key(&key) {
                // 1. Shape text
                let metrics = Metrics::new(scaled_size, scaled_size * 1.2);
                let mut buffer = Buffer::new(&mut self.font_system, metrics);
                // Very large wrap size to prevent any line wrapping for single-line labels
                buffer.set_size(&mut self.font_system, Some(10000.0), Some(10000.0));
                let weight = if cmd.bold { Weight::BOLD } else { Weight::NORMAL };
                let family = if cmd.icon { &self.icon_family } else { &self.font_family };
                let attrs = Attrs::new().family(Family::Name(family)).weight(weight);
                buffer.set_text(&mut self.font_system, &cmd.text, &attrs, Shaping::Advanced);
                buffer.shape_until_scroll(&mut self.font_system, false);

                // 2. Measure exact bounds. Stable, glyph-independent line metrics
                // for vertical centering so the baseline doesn't jump with
                // ascenders/descenders.
                let mut min_x = i32::MAX;
                let mut min_y = i32::MAX;
                let mut max_x = i32::MIN;
                let mut max_y = i32::MIN;
                let mut has_glyphs = false;
                let mut line_top = 0.0f32;
                let mut line_height = 0.0f32;
                for run in buffer.layout_runs() {
                    line_top = run.line_top;
                    line_height = run.line_height;
                    for glyph in run.glyphs {
                        let physical = glyph.physical((0.0, run.line_y), 1.0);
                        let img_opt = self.swash_cache.get_image(&mut self.font_system, physical.cache_key);
                        let img = match img_opt.as_ref() {
                            Some(img) => img,
                            None => continue,
                        };
                        let gw = img.placement.width;
                        let gh = img.placement.height;
                        if gw == 0 || gh == 0 {
                            continue;
                        }
                        let left = physical.x + img.placement.left;
                        let top = physical.y - img.placement.top;
                        min_x = min_x.min(left);
                        min_y = min_y.min(top);
                        max_x = max_x.max(left + gw as i32);
                        max_y = max_y.max(top + gh as i32);
                        has_glyphs = true;
                    }
                }
                if !has_glyphs {
                    continue; // empty / whitespace — nothing to draw (not cached)
                }
                let content_w = (max_x - min_x) as u32;
                let content_h = (max_y - min_y) as u32;
                if content_w == 0 || content_h == 0 {
                    continue;
                }

                // 3. Blit glyph images into a CPU pixel buffer (alpha mask).
                let align = |v: u32| v.div_ceil(256) * 256;
                let stride = align(content_w);
                let mut pixels = vec![0u8; (stride * content_h) as usize];
                for run in buffer.layout_runs() {
                    for glyph in run.glyphs {
                        let physical = glyph.physical((0.0, run.line_y), 1.0);
                        let img_opt = self.swash_cache.get_image(&mut self.font_system, physical.cache_key);
                        let img = match img_opt.as_ref() {
                            Some(img) => img,
                            None => continue,
                        };
                        let gw = img.placement.width;
                        let gh = img.placement.height;
                        if gw == 0 || gh == 0 {
                            continue;
                        }
                        let dst_x = (physical.x + img.placement.left - min_x) as u32;
                        let dst_y = (physical.y - img.placement.top - min_y) as u32;
                        for py in 0..gh {
                            for px in 0..gw {
                                let src_idx = (py * gw + px) as usize;
                                let dst_idx = ((dst_y + py) * stride + (dst_x + px)) as usize;
                                if dst_idx < pixels.len() {
                                    pixels[dst_idx] = pixels[dst_idx].saturating_add(img.data[src_idx]);
                                }
                            }
                        }
                    }
                }

                // 4. Upload as an alpha-mask texture + bind group, cached by key.
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("text_label"),
                    size: wgpu::Extent3d { width: content_w, height: content_h, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &pixels,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(stride),
                        rows_per_image: Some(content_h),
                    },
                    wgpu::Extent3d { width: content_w, height: content_h, depth_or_array_layers: 1 },
                );
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("text_label_bind_group"),
                    layout: &self.bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: self.uniform_buffer.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                        wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                    ],
                });

                self.label_cache.insert(
                    key.clone(),
                    CachedLabel {
                        _texture: texture,
                        bind_group,
                        content_w,
                        content_h,
                        line_top,
                        line_height,
                        min_y,
                        last_used: self.frame,
                    },
                );
            }

            let cl = self.label_cache.get_mut(&key).unwrap();
            cl.last_used = self.frame;
            // Pull the cached geometry; the centering math below is unchanged.
            let (content_w, content_h, line_top, line_height, min_y) =
                (cl.content_w, cl.content_h, cl.line_top, cl.line_height, cl.min_y);

            // 5. Build quad. Box mode centers within `(w, h)`: horizontally per
            // `align` on the ink width; vertically on the **stable line box** so
            // the baseline doesn't shift when glyphs gain ascenders/descenders.
            let screen_w = content_w as f32 / scale;
            let screen_h = content_h as f32 / scale;
            let (screen_x, screen_y) = if cmd.centered {
                let x = cmd.x
                    + match cmd.align {
                        TextAlign::Start => 0.0,
                        TextAlign::Center => (cmd.w - screen_w) * 0.5,
                        TextAlign::End => cmd.w - screen_w,
                    };
                let line_box_top = cmd.y + (cmd.h - line_height / scale) * 0.5;
                let ink_offset = (min_y as f32 - line_top) / scale;
                (x, line_box_top + ink_offset)
            } else {
                (cmd.x, cmd.y)
            };

            vertices.push(TextVertex { position: [screen_x, screen_y], texcoord: [0.0, 0.0], color: cmd.color });
            vertices.push(TextVertex { position: [screen_x + screen_w, screen_y], texcoord: [1.0, 0.0], color: cmd.color });
            vertices.push(TextVertex { position: [screen_x + screen_w, screen_y + screen_h], texcoord: [1.0, 1.0], color: cmd.color });
            vertices.push(TextVertex { position: [screen_x, screen_y + screen_h], texcoord: [0.0, 1.0], color: cmd.color });
            indices.push(base);
            indices.push(base + 1);
            indices.push(base + 2);
            indices.push(base);
            indices.push(base + 2);
            indices.push(base + 3);
            keys.push((key, cmd.clip));
            base += 4;
        }

        // Evict labels unused for a while (dynamic text) so GPU textures don't leak.
        let frame = self.frame;
        self.label_cache.retain(|_, cl| frame.saturating_sub(cl.last_used) <= LABEL_EVICT_AFTER);

        (vertices, indices, keys)
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if self.commands.is_empty() {
            return;
        }

        let (vertices, indices, keys) = self.build_labels(device, queue);

        if vertices.is_empty() {
            self.commands.clear();
            return;
        }

        let vertex_data = bytemuck::cast_slice(&vertices);
        let index_data = bytemuck::cast_slice(&indices);

        // Stage + copy via the encoder (NOT queue.write_buffer): the showcase
        // renders text in two passes (base, then overlay) into one buffer, and
        // copies are encoder commands that interleave with the draws — so the base
        // pass draws before the overlay pass overwrites the buffer. A queue write
        // would land before *all* draws, corrupting the base-layer text whenever
        // an overlay (e.g. toasts) adds a second pass.
        let staging_v = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("text_vertex_staging"),
            contents: vertex_data,
            usage: wgpu::BufferUsages::COPY_SRC,
        });
        let staging_i = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("text_index_staging"),
            contents: index_data,
            usage: wgpu::BufferUsages::COPY_SRC,
        });
        encoder.copy_buffer_to_buffer(&staging_v, 0, &self.vertex_buffer, 0, vertex_data.len() as u64);
        encoder.copy_buffer_to_buffer(&staging_i, 0, &self.index_buffer, 0, index_data.len() as u64);

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
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        rpass.set_pipeline(&self.pipeline);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        // Draw each label with its cached bind group (looked up by key), scissored
        // to its clip rect when one is set (e.g. a scrolled viewport).
        let scale = self.scale_factor as f32;
        for (i, (key, clip)) in keys.iter().enumerate() {
            let Some(cl) = self.label_cache.get(key) else { continue };
            match clip {
                None => rpass.set_scissor_rect(0, 0, self.target_size[0], self.target_size[1]),
                Some(c) => {
                    let (x, y, w, h) = scissor_px(*c, scale, self.target_size);
                    if w == 0 || h == 0 {
                        continue; // fully clipped — nothing visible
                    }
                    rpass.set_scissor_rect(x, y, w, h);
                }
            }
            rpass.set_bind_group(0, &cl.bind_group, &[]);
            let start = i as u32 * 6;
            rpass.draw_indexed(start..start + 6, 0, 0..1);
        }

        self.commands.clear();
    }
}
