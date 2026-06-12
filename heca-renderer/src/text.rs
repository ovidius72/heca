use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight};
use heca_grid_ui::scene::TextAlign;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use wgpu::util::DeviceExt;

use crate::font;

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

#[derive(Clone)]
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
    font_family: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TextCacheKey {
    text: String,
    scaled_font_size_bits: u32,
    bold: bool,
    italic: bool,
    icon: bool,
    font_family: Option<String>,
}

struct CachedTextLabel {
    bind_group: wgpu::BindGroup,
    screen_w: f32,
    screen_h: f32,
    min_y: i32,
    line_top: f32,
    line_height: f32,
    last_used_frame: u64,
}

pub struct TextStyle<'a> {
    pub color: [f32; 4],
    pub bold: bool,
    pub italic: bool,
    pub faux_italic: bool,
    pub font_family: Option<&'a str>,
}

pub struct TextBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

struct LabelCache<V> {
    buckets: HashMap<u64, Vec<(TextCacheKey, V)>>,
}

impl<V> LabelCache<V> {
    fn new() -> Self {
        Self {
            buckets: HashMap::new(),
        }
    }

    fn clear(&mut self) {
        self.buckets.clear();
    }

    fn entry_count(&self) -> usize {
        self.buckets.values().map(Vec::len).sum()
    }

    fn find_index_for_command(&self, fingerprint: u64, cmd: &TextCommand, scale: f32) -> Option<usize> {
        self.buckets.get(&fingerprint).and_then(|bucket| {
            bucket
                .iter()
                .position(|(key, _)| TextRenderer::cache_key_matches_command(key, cmd, scale))
        })
    }

    fn insert(&mut self, fingerprint: u64, key: TextCacheKey, value: V) -> usize {
        let bucket = self.buckets.entry(fingerprint).or_default();
        bucket.push((key, value));
        bucket.len() - 1
    }

    fn get_mut(&mut self, fingerprint: u64, index: usize) -> Option<&mut V> {
        self.buckets
            .get_mut(&fingerprint)
            .and_then(|bucket| bucket.get_mut(index))
            .map(|(_, value)| value)
    }

    fn prune_with_limits(
        &mut self,
        max_cached_labels: usize,
        current_frame: u64,
        max_idle_frames: u64,
        mut last_used_frame: impl FnMut(&V) -> u64,
    ) {
        if self.entry_count() <= max_cached_labels {
            return;
        }

        let oldest_frame = current_frame.saturating_sub(max_idle_frames);
        self.buckets.retain(|_, bucket| {
            bucket.retain(|(_, value)| last_used_frame(value) >= oldest_frame);
            !bucket.is_empty()
        });
    }
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
    commands: Vec<TextCommand>,
    label_cache: LabelCache<CachedTextLabel>,
    frame_index: u64,
    _atlas_size: (u32, u32),
    font_family: String,
    icon_family: String,
}

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
        font_system
            .db_mut()
            .load_font_data(font::DEFAULT_TERMINAL_BYTES.to_vec());
        font_system
            .db_mut()
            .load_font_data(font::DEFAULT_TERMINAL_BOLD_BYTES.to_vec());
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
            commands: Vec::new(),
            label_cache: LabelCache::new(),
            frame_index: 0,
            _atlas_size: (0, 0),
            font_family: heca_grid_ui::font::DEFAULT_MONO_FAMILY.to_string(),
            icon_family: heca_grid_ui::font::ICON_FONT_FAMILY.to_string(),
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
        if (self.scale_factor - scale).abs() > f64::EPSILON {
            self.scale_factor = scale;
            self.label_cache.clear();
        }
    }

    pub fn set_font_family(&mut self, family: &str) {
        if self.font_family != family {
            self.font_family = family.to_string();
            self.label_cache.clear();
        }
    }

    /// Queue text with its top-left at `(x, y)` (logical px) — the simple
    /// point-positioned form used throughout the app (sidebar, chrome, …).
    pub fn queue_text(&mut self, text: &str, x: f32, y: f32, font_size: f32, color: [f32; 4]) {
        self.queue_text_styled(text, x, y, font_size, color, false);
    }

    /// Queue text with explicit weight at `(x, y)` (logical px).
    pub fn queue_text_styled(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        color: [f32; 4],
        bold: bool,
    ) {
        self.queue_text_with_style(
            text,
            x,
            y,
            font_size,
            TextStyle {
                color,
                bold,
                italic: false,
                faux_italic: false,
                font_family: None,
            },
        );
    }

    pub fn queue_text_with_style(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        style: TextStyle<'_>,
    ) {
        self.commands.push(TextCommand {
            text: text.to_string(),
            x,
            y,
            w: 0.0,
            h: 0.0,
            font_size,
            color: style.color,
            bold: style.bold,
            italic: style.italic,
            faux_italic: style.faux_italic,
            align: TextAlign::Start,
            centered: false,
            line_box: false,
            icon: false,
            font_family: style.font_family.map(ToOwned::to_owned),
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
        self.queue_text_in_box_with_style(
            text,
            TextBox { x, y, w, h },
            font_size,
            TextStyle {
                color,
                bold,
                italic: false,
                faux_italic: false,
                font_family: None,
            },
            align,
            icon,
        );
    }

    pub fn queue_text_in_box_with_style(
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
            centered: true,
            line_box: false,
            icon,
            font_family: style.font_family.map(ToOwned::to_owned),
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
            font_family: style.font_family.map(ToOwned::to_owned),
        });
    }

    fn cache_key_for_command(&self, cmd: &TextCommand, scale: f32) -> TextCacheKey {
        TextCacheKey {
            text: cmd.text.clone(),
            scaled_font_size_bits: (cmd.font_size * scale).to_bits(),
            bold: cmd.bold,
            italic: cmd.italic,
            icon: cmd.icon,
            font_family: cmd.font_family.clone(),
        }
    }

    fn cache_fingerprint_for_command(cmd: &TextCommand, scale: f32) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        cmd.text.hash(&mut hasher);
        (cmd.font_size * scale).to_bits().hash(&mut hasher);
        cmd.bold.hash(&mut hasher);
        cmd.italic.hash(&mut hasher);
        cmd.icon.hash(&mut hasher);
        cmd.font_family.hash(&mut hasher);
        hasher.finish()
    }

    fn cache_key_matches_command(key: &TextCacheKey, cmd: &TextCommand, scale: f32) -> bool {
        key.text == cmd.text
            && key.scaled_font_size_bits == (cmd.font_size * scale).to_bits()
            && key.bold == cmd.bold
            && key.italic == cmd.italic
            && key.icon == cmd.icon
            && key.font_family == cmd.font_family
    }

    fn rasterize_label(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        cmd: &TextCommand,
        scale: f32,
    ) -> Option<CachedTextLabel> {
        let scaled_size = cmd.font_size * scale;
        let metrics = Metrics::new(scaled_size, scaled_size * 1.2);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        // Very large wrap size to prevent any line wrapping for single-line labels
        buffer.set_size(&mut self.font_system, Some(10000.0), Some(10000.0));
        let weight = if cmd.bold {
            Weight::BOLD
        } else {
            Weight::NORMAL
        };
        let family = if cmd.icon {
            &self.icon_family
        } else {
            cmd.font_family.as_deref().unwrap_or(&self.font_family)
        };
        let attrs = Attrs::new()
            .family(Family::Name(family))
            .weight(weight)
            .style(if cmd.italic { Style::Italic } else { Style::Normal });
        buffer.set_text(&mut self.font_system, &cmd.text, &attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut self.font_system, false);

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
                let img_opt = self
                    .swash_cache
                    .get_image(&mut self.font_system, physical.cache_key);
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
                let right = left + gw as i32;
                let bottom = top + gh as i32;

                min_x = min_x.min(left);
                min_y = min_y.min(top);
                max_x = max_x.max(right);
                max_y = max_y.max(bottom);
                has_glyphs = true;
            }
        }

        if !has_glyphs {
            return None;
        }

        let content_w = (max_x - min_x) as u32;
        let content_h = (max_y - min_y) as u32;
        if content_w == 0 || content_h == 0 {
            return None;
        }

        let align = |v: u32| v.div_ceil(256) * 256;
        let stride = align(content_w);
        let mut pixels = vec![0u8; (stride * content_h) as usize];

        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                let physical = glyph.physical((0.0, run.line_y), 1.0);
                let img_opt = self
                    .swash_cache
                    .get_image(&mut self.font_system, physical.cache_key);
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

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("text_label"),
            size: wgpu::Extent3d {
                width: content_w,
                height: content_h,
                depth_or_array_layers: 1,
            },
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
            wgpu::Extent3d {
                width: content_w,
                height: content_h,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("text_label_bind_group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        Some(CachedTextLabel {
            bind_group,
            screen_w: content_w as f32 / scale,
            screen_h: content_h as f32 / scale,
            min_y,
            line_top,
            line_height,
            last_used_frame: self.frame_index,
        })
    }

    fn prune_label_cache(&mut self) {
        self.prune_label_cache_with_limits(4096, 180);
    }

    fn prune_label_cache_with_limits(&mut self, max_cached_labels: usize, max_idle_frames: u64) {
        self.label_cache.prune_with_limits(
            max_cached_labels,
            self.frame_index,
            max_idle_frames,
            |label| label.last_used_frame,
        );
    }

    /// Render all queued text. Returns (vertices, indices, labels) for drawing.
    fn build_labels(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> (Vec<TextVertex>, Vec<u16>, Vec<wgpu::BindGroup>) {
        if self.commands.is_empty() {
            return (Vec::new(), Vec::new(), Vec::new());
        }

        let scale = self.scale_factor as f32;
        let commands = std::mem::take(&mut self.commands);
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut labels = Vec::new();
        let mut base = 0u16;

        for cmd in &commands {
            let fingerprint = Self::cache_fingerprint_for_command(cmd, scale);
            let mut label_index = self
                .label_cache
                .find_index_for_command(fingerprint, cmd, scale);

            if label_index.is_none() {
                let Some(label) = self.rasterize_label(device, queue, cmd, scale) else {
                    continue;
                };
                let key = self.cache_key_for_command(cmd, scale);
                label_index = Some(self.label_cache.insert(fingerprint, key, label));
            }

            let Some(label_idx) = label_index else {
                continue;
            };
            let Some(label) = self.label_cache.get_mut(fingerprint, label_idx) else {
                continue;
            };
            label.last_used_frame = self.frame_index;

            // 5. Build quad. Box mode centers within `(w, h)`: horizontally per
            // `align` on the ink width; vertically on the **stable line box** (not
            // the ink box) so the baseline doesn't shift when glyphs gain
            // ascenders/descenders (p, q, g, b, t). The ink box is then placed at
            // its measured offset from the line top, so existing glyphs stay put.
            let (screen_x, screen_y) = if cmd.centered || cmd.line_box {
                let x = cmd.x
                    + match cmd.align {
                        TextAlign::Start => 0.0,
                        TextAlign::Center => (cmd.w - label.screen_w) * 0.5,
                        TextAlign::End => cmd.w - label.screen_w,
                    };
                let line_box_top = cmd.y + (cmd.h - label.line_height / scale) * 0.5;
                let ink_offset = (label.min_y as f32 - label.line_top) / scale;
                (x, line_box_top + ink_offset)
            } else {
                (cmd.x, cmd.y)
            };

            let faux_italic_skew = if cmd.faux_italic {
                (label.screen_h * 0.14).max(1.0)
            } else {
                0.0
            };
            let top_x = screen_x + faux_italic_skew * 0.5;
            let bottom_x = screen_x - faux_italic_skew * 0.5;

            vertices.push(TextVertex {
                position: [top_x, screen_y],
                texcoord: [0.0, 0.0],
                color: cmd.color,
            });
            vertices.push(TextVertex {
                position: [top_x + label.screen_w, screen_y],
                texcoord: [1.0, 0.0],
                color: cmd.color,
            });
            vertices.push(TextVertex {
                position: [bottom_x + label.screen_w, screen_y + label.screen_h],
                texcoord: [1.0, 1.0],
                color: cmd.color,
            });
            vertices.push(TextVertex {
                position: [bottom_x, screen_y + label.screen_h],
                texcoord: [0.0, 1.0],
                color: cmd.color,
            });
            indices.push(base);
            indices.push(base + 1);
            indices.push(base + 2);
            indices.push(base);
            indices.push(base + 2);
            indices.push(base + 3);

            labels.push(label.bind_group.clone());

            base += 4;
        }

        (vertices, indices, labels)
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        self.render_clipped(device, queue, view, encoder, None);
    }

    pub fn render_clipped(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        clip_rect: Option<(u32, u32, u32, u32)>,
    ) {
        if self.commands.is_empty() {
            return;
        }

        self.frame_index = self.frame_index.wrapping_add(1);
        let (vertices, indices, labels) = self.build_labels(device, queue);
        self.prune_label_cache();

        if vertices.is_empty() {
            return;
        }

        let vertex_data = bytemuck::cast_slice(&vertices);
        let index_data = bytemuck::cast_slice(&indices);

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

        encoder.copy_buffer_to_buffer(
            &staging_v,
            0,
            &self.vertex_buffer,
            0,
            vertex_data.len() as u64,
        );
        encoder.copy_buffer_to_buffer(
            &staging_i,
            0,
            &self.index_buffer,
            0,
            index_data.len() as u64,
        );

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
        if let Some((x, y, w, h)) = clip_rect {
            rpass.set_scissor_rect(x, y, w, h);
        }

        // Draw each label with its own bind group
        for (i, label) in labels.iter().enumerate() {
            rpass.set_bind_group(0, label, &[]);
            let start = i as u32 * 6;
            rpass.draw_indexed(start..start + 6, 0, 0..1);
        }

    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(text: &str, scale: f32, font_family: Option<&str>) -> (TextCommand, TextCacheKey, u64) {
        let cmd = TextCommand {
            text: text.to_string(),
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
            font_size: 16.0,
            color: [1.0, 1.0, 1.0, 1.0],
            bold: false,
            italic: false,
            faux_italic: false,
            align: TextAlign::Start,
            centered: false,
            line_box: false,
            icon: false,
            font_family: font_family.map(ToOwned::to_owned),
        };
        let key = TextCacheKey {
            text: cmd.text.clone(),
            scaled_font_size_bits: (cmd.font_size * scale).to_bits(),
            bold: cmd.bold,
            italic: cmd.italic,
            icon: cmd.icon,
            font_family: cmd.font_family.clone(),
        };
        let fingerprint = TextRenderer::cache_fingerprint_for_command(&cmd, scale);
        (cmd, key, fingerprint)
    }

    #[test]
    fn text_label_cache_hits_and_misses_by_command_identity() {
        let scale = 1.0;
        let (cmd_hit, key_hit, fp_hit) = command("cache-hit", scale, Some("Maple Mono Normal NF"));
        let (cmd_miss, _key_miss, fp_miss) = command("cache-miss", scale, Some("Maple Mono Normal NF"));
        let mut cache = LabelCache::new();

        cache.insert(fp_hit, key_hit, 7u64);

        assert_eq!(cache.entry_count(), 1);
        assert_eq!(cache.find_index_for_command(fp_hit, &cmd_hit, scale), Some(0));
        assert_eq!(cache.find_index_for_command(fp_miss, &cmd_miss, scale), None);
    }

    #[test]
    fn text_label_cache_distinguishes_scale_and_font_variants() {
        let (cmd_a, _key_a, fp_a) = command("same-text", 1.0, Some("Maple Mono Normal NF"));
        let (_cmd_b, _key_b, fp_b) = command("same-text", 2.0, Some("Maple Mono Normal NF"));
        let (_cmd_c, _key_c, fp_c) = command("same-text", 1.0, Some("Geist Mono"));

        assert!(TextRenderer::cache_key_matches_command(
            &TextCacheKey {
                text: cmd_a.text.clone(),
                scaled_font_size_bits: (cmd_a.font_size * 1.0).to_bits(),
                bold: cmd_a.bold,
                italic: cmd_a.italic,
                icon: cmd_a.icon,
                font_family: cmd_a.font_family.clone(),
            },
            &cmd_a,
            1.0
        ));
        assert_ne!(fp_a, fp_b, "scale changes must invalidate cached labels");
        assert_ne!(fp_a, fp_c, "font-family changes must invalidate cached labels");
    }

    #[test]
    fn text_label_cache_prunes_idle_entries() {
        let scale = 1.0;
        let (_cmd_a, key_a, fp_a) = command("one", scale, None);
        let (_cmd_b, key_b, fp_b) = command("two", scale, None);
        let (_cmd_c, key_c, fp_c) = command("three", scale, None);
        let mut cache = LabelCache::new();

        cache.insert(fp_a, key_a, 0u64);
        cache.insert(fp_b, key_b, 0u64);
        cache.insert(fp_c, key_c, 10u64);
        assert_eq!(cache.entry_count(), 3);

        cache.prune_with_limits(2, 10, 1, |last_used| *last_used);

        assert_eq!(cache.entry_count(), 1);
    }

    #[test]
    fn text_label_cache_can_be_cleared_for_renderer_invalidation() {
        let scale = 1.0;
        let (_cmd, key, fp) = command("clear-me", scale, Some("Maple Mono Normal NF"));
        let mut cache = LabelCache::new();
        cache.insert(fp, key, 5u64);

        assert_eq!(cache.entry_count(), 1);
        cache.clear();
        assert_eq!(cache.entry_count(), 0);
    }
}
