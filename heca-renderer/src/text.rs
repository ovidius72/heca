use cosmic_text::{Attrs, Buffer, Color as CosmicColor, Family, FontSystem, Metrics, Shaping, SwashCache};
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
    x: f32,
    y: f32,
    font_size: f32,
    color: [f32; 4],
}

/// Per-command atlas entry.
struct CmdAtlas {
    cmd: TextCommand,
    atlas_x: u32,
    atlas_y: u32,
    width: u32,
    height: u32,
    min_x: i32,
    min_y: i32,
}

/// GPU text renderer powered by cosmic-text.
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
    atlas_texture: Option<wgpu::Texture>,
    atlas_view: Option<wgpu::TextureView>,
    atlas_bind_group: Option<wgpu::BindGroup>,
    atlas_size: (u32, u32),
    font_family: String,
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

        Self {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
            pipeline,
            vertex_buffer,
            index_buffer,
            bind_group_layout,
            uniform_buffer,
            sampler,
            scale_factor: 1.0,
            commands: Vec::new(),
            atlas_texture: None,
            atlas_view: None,
            atlas_bind_group: None,
            atlas_size: (0, 0),
            font_family: "monospace".to_string(),
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

    pub fn set_font_family(&mut self, family: &str) {
        self.font_family = family.to_string();
    }

    pub fn queue_text(&mut self, text: &str, x: f32, y: f32, font_size: f32, color: [f32; 4]) {
        self.commands.push(TextCommand {
            text: text.to_string(),
            x,
            y,
            font_size,
            color,
        });
    }

    fn build_atlas(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> (Vec<TextVertex>, Vec<u16>) {
        if self.commands.is_empty() {
            return (Vec::new(), Vec::new());
        }

        // ── PASS 1: shape each command and measure exact glyph bounds ──
        let mut entries: Vec<CmdAtlas> = Vec::new();
        let mut atlas_cursor_x = 0u32;
        let mut atlas_cursor_y = 0u32;
        let mut row_height = 0u32;
        let mut total_width = 0u32;
        const PAD: u32 = 4; // padding around each command in atlas
        const MAX_ATLAS_W: u32 = 2048;

        for cmd in &self.commands {
            let scaled_size = cmd.font_size * self.scale_factor as f32;
            let metrics = Metrics::new(scaled_size, scaled_size * 1.2);
            let mut buffer = Buffer::new(&mut self.font_system, metrics);
            let attrs = Attrs::new().family(Family::Name(&self.font_family));
            buffer.set_text(&mut self.font_system, &cmd.text, &attrs, Shaping::Advanced);
            buffer.shape_until_scroll(&mut self.font_system, false);

            // Measure exact pixel bounds by inspecting every glyph image
            let mut min_x = i32::MAX;
            let mut min_y = i32::MAX;
            let mut max_x = i32::MIN;
            let mut max_y = i32::MIN;
            let mut has_glyphs = false;

            for run in buffer.layout_runs() {
                for glyph in run.glyphs {
                    let physical = glyph.physical((0.0, 0.0), 1.0);
                    let img_opt = self.swash_cache.get_image(&mut self.font_system, physical.cache_key);
                    let img = match img_opt.as_ref() {
                        Some(img) => img,
                        None => continue,
                    };
                    let gw = img.placement.width;
                    let gh = img.placement.height;
                    if gw == 0 || gh == 0 { continue; }

                    let left = physical.x + img.placement.left;
                    let top = run.line_y as i32 + physical.y + img.placement.top;
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
                // No renderable glyphs — skip but still create a tiny entry so indices stay aligned
                entries.push(CmdAtlas {
                    cmd: TextCommand {
                        text: cmd.text.clone(),
                        x: cmd.x,
                        y: cmd.y,
                        font_size: cmd.font_size,
                        color: cmd.color,
                    },
                    atlas_x: 0,
                    atlas_y: 0,
                    width: 1,
                    height: 1,
                    min_x: 0,
                    min_y: 0,
                });
                continue;
            }

            let cmd_w = (max_x - min_x) as u32 + PAD * 2;
            let cmd_h = (max_y - min_y) as u32 + PAD * 2;

            // Simple row packing
            if atlas_cursor_x + cmd_w > MAX_ATLAS_W {
                atlas_cursor_x = 0;
                atlas_cursor_y += row_height;
                row_height = 0;
            }

            entries.push(CmdAtlas {
                cmd: TextCommand {
                    text: cmd.text.clone(),
                    x: cmd.x,
                    y: cmd.y,
                    font_size: cmd.font_size,
                    color: cmd.color,
                },
                atlas_x: atlas_cursor_x,
                atlas_y: atlas_cursor_y,
                width: cmd_w,
                height: cmd_h,
                min_x,
                min_y,
            });

            atlas_cursor_x += cmd_w;
            row_height = row_height.max(cmd_h);
            total_width = total_width.max(atlas_cursor_x);
        }

        let atlas_h = (atlas_cursor_y + row_height).max(1);
        let atlas_w = total_width.max(1);
        self.atlas_size = (atlas_w, atlas_h);

        // Align stride for GPU texture upload
        let align = |v: u32| ((v + 255) / 256) * 256;
        let stride = align(atlas_w);
        let mut atlas = vec![0u8; (stride * atlas_h) as usize];

        // ── PASS 2: draw each command into the atlas using Buffer::draw ──
        // Split mutable borrows so we can pass font_system + swash_cache together
        let font_system = &mut self.font_system;
        let swash_cache = &mut self.swash_cache;

        for entry in &entries {
            if entry.width <= 1 && entry.height <= 1 {
                continue; // empty command
            }

            let scaled_size = entry.cmd.font_size * self.scale_factor as f32;
            let metrics = Metrics::new(scaled_size, scaled_size * 1.2);
            let mut buffer = Buffer::new(font_system, metrics);
            let attrs = Attrs::new().family(Family::Name(&self.font_family));
            buffer.set_text(font_system, &entry.cmd.text, &attrs, Shaping::Advanced);
            buffer.shape_until_scroll(font_system, false);

            let offset_x = entry.atlas_x + PAD;
            let offset_y = entry.atlas_y + PAD;
            let atlas_w_local = atlas_w;
            let atlas_h_local = atlas_h;
            let stride_local = stride;
            let min_x = entry.min_x;
            let min_y = entry.min_y;

            buffer.draw(
                font_system,
                swash_cache,
                CosmicColor::rgb(0xFF, 0xFF, 0xFF),
                |x: i32, y: i32, w: u32, h: u32, color: CosmicColor| {
                    let a = color.a();
                    if a == 0 {
                        return;
                    }
                    let base_x = offset_x as i32 + (x - min_x);
                    let base_y = offset_y as i32 + (y - min_y);
                    for dy in 0..h {
                        for dx in 0..w {
                            let px = base_x + dx as i32;
                            let py = base_y + dy as i32;
                            if px >= 0 && px < atlas_w_local as i32 && py >= 0 && py < atlas_h_local as i32 {
                                let idx = (py as u32 * stride_local + px as u32) as usize;
                                atlas[idx] = atlas[idx].saturating_add(a);
                            }
                        }
                    }
                },
            );
        }

        // ── PASS 3: build vertex quads (one per command) ──
        let scale = self.scale_factor as f32;
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut base = 0u16;

        for entry in &entries {
            if entry.width <= 1 && entry.height <= 1 {
                // Empty command — push degenerate quad
                vertices.push(TextVertex {
                    position: [0.0, 0.0],
                    texcoord: [0.0, 0.0],
                    color: [0.0, 0.0, 0.0, 0.0],
                });
                vertices.push(TextVertex {
                    position: [0.0, 0.0],
                    texcoord: [0.0, 0.0],
                    color: [0.0, 0.0, 0.0, 0.0],
                });
                vertices.push(TextVertex {
                    position: [0.0, 0.0],
                    texcoord: [0.0, 0.0],
                    color: [0.0, 0.0, 0.0, 0.0],
                });
                vertices.push(TextVertex {
                    position: [0.0, 0.0],
                    texcoord: [0.0, 0.0],
                    color: [0.0, 0.0, 0.0, 0.0],
                });
                indices.push(base);
                indices.push(base + 1);
                indices.push(base + 2);
                indices.push(base);
                indices.push(base + 2);
                indices.push(base + 3);
                base += 4;
                continue;
            }

            let x0 = entry.cmd.x + entry.min_x as f32 / scale;
            let y0 = entry.cmd.y + entry.min_y as f32 / scale;
            let x1 = x0 + entry.width as f32 / scale;
            let y1 = y0 + entry.height as f32 / scale;

            let u0 = entry.atlas_x as f32 / atlas_w as f32;
            let u1 = (entry.atlas_x + entry.width) as f32 / atlas_w as f32;
            let v0 = entry.atlas_y as f32 / atlas_h as f32;
            let v1 = (entry.atlas_y + entry.height) as f32 / atlas_h as f32;

            vertices.push(TextVertex { position: [x0, y0], texcoord: [u0, v0], color: entry.cmd.color });
            vertices.push(TextVertex { position: [x1, y0], texcoord: [u1, v0], color: entry.cmd.color });
            vertices.push(TextVertex { position: [x1, y1], texcoord: [u1, v1], color: entry.cmd.color });
            vertices.push(TextVertex { position: [x0, y1], texcoord: [u0, v1], color: entry.cmd.color });
            indices.push(base);
            indices.push(base + 1);
            indices.push(base + 2);
            indices.push(base);
            indices.push(base + 2);
            indices.push(base + 3);
            base += 4;
        }

        // Upload atlas texture
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("text_atlas"),
            size: wgpu::Extent3d {
                width: atlas_w,
                height: atlas_h,
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
            &atlas,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(atlas_h),
            },
            wgpu::Extent3d {
                width: atlas_w,
                height: atlas_h,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("text_bind_group"),
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

        self.atlas_texture = Some(texture);
        self.atlas_view = Some(view);
        self.atlas_bind_group = Some(bind_group);

        (vertices, indices)
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

        let (vertices, indices) = self.build_atlas(device, queue);

        if vertices.is_empty() {
            self.commands.clear();
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
        if let Some(ref bg) = self.atlas_bind_group {
            rpass.set_bind_group(0, bg, &[]);
        }
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..indices.len() as u32, 0, 0..1);

        self.commands.clear();
    }
}
