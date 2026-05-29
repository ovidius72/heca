use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, SwashCache};
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

/// GPU text renderer powered by cosmic-text.
/// Phase 1: simple per-frame atlas approach.
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

    /// Queue a line of text for rendering (collected until `render()` is called).
    pub fn queue_text(&mut self, text: &str, x: f32, y: f32, font_size: f32, color: [f32; 4]) {
        self.commands.push(TextCommand {
            text: text.to_string(),
            x,
            y,
            font_size,
            color,
        });
    }

    /// Build the atlas, upload to GPU, and return vertices/indices for drawing.
    fn build_atlas(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> (Vec<TextVertex>, Vec<u16>) {
        if self.commands.is_empty() {
            return (Vec::new(), Vec::new());
        }

        // First pass: measure all text blocks to size the atlas
        let mut measurements = Vec::new();
        let mut total_height = 0u32;
        let mut max_width = 0u32;

        for cmd in &self.commands {
            let scaled_size = cmd.font_size * self.scale_factor as f32;
            let metrics = Metrics::new(scaled_size, scaled_size * 1.2);
            let mut buffer = Buffer::new(&mut self.font_system, metrics);
            buffer.set_text(&mut self.font_system, &cmd.text, &Attrs::new(), Shaping::Advanced);
            buffer.shape_until_scroll(&mut self.font_system, false);

            let mut cmd_w = 0f32;
            let mut cmd_h = 0f32;
            for run in buffer.layout_runs() {
                let mut line_w = 0f32;
                for glyph in run.glyphs {
                    line_w += glyph.w;
                }
                cmd_w = cmd_w.max(line_w);
                cmd_h += run.line_height;
            }
            let w = cmd_w.ceil().max(1.0) as u32;
            let h = cmd_h.ceil().max(1.0) as u32;
            measurements.push((w, h));
            max_width = max_width.max(w);
            total_height += h;
        }

        let atlas_w = max_width.max(1);
        let atlas_h = total_height.max(1);
        self.atlas_size = (atlas_w, atlas_h);

        // Align stride for GPU texture upload
        let align = |v: u32| ((v + 255) / 256) * 256;
        let stride = align(atlas_w);

        // Allocate atlas buffer with aligned stride
        let mut atlas = vec![0u8; (stride * atlas_h) as usize];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut current_y = 0u32;

        // Second pass: rasterize each command into the atlas
        for (i, cmd) in self.commands.iter().enumerate() {
            let (cmd_w, cmd_h) = measurements[i];
            let scaled_size = cmd.font_size * self.scale_factor as f32;
            let metrics = Metrics::new(scaled_size, scaled_size * 1.2);
            let mut buffer = Buffer::new(&mut self.font_system, metrics);
            buffer.set_text(&mut self.font_system, &cmd.text, &Attrs::new(), Shaping::Advanced);
            buffer.shape_until_scroll(&mut self.font_system, false);

            // Rasterize glyphs into atlas at (0, current_y), using stride for row width
            for run in buffer.layout_runs() {
                let line_y = run.line_y;
                for glyph in run.glyphs {
                    let physical = glyph.physical((0.0, 0.0), 1.0);
                    if let Some(img) = self.swash_cache.get_image(&mut self.font_system, physical.cache_key) {
                        let gx = physical.x as i32;
                        let gy = current_y as i32 + (line_y + physical.y as f32) as i32;
                        let gw = img.placement.width;
                        let gh = img.placement.height;
                        for py in 0..gh {
                            for px in 0..gw {
                                let src_idx = (py * gw + px) as usize;
                                let dst_x = gx + px as i32;
                                let dst_y = gy + py as i32;
                                if dst_x >= 0 && dst_x < stride as i32 && dst_y >= 0 && dst_y < atlas_h as i32 {
                                    let dst_idx = (dst_y * stride as i32 + dst_x) as usize;
                                    atlas[dst_idx] = atlas[dst_idx].saturating_add(img.data[src_idx]);
                                }
                            }
                        }
                    }
                }
            }

            // Build quad for this text block
            let base = vertices.len() as u16;
            let x1 = cmd.x;
            let y1 = cmd.y;
            let x2 = cmd.x + (cmd_w as f32 / self.scale_factor as f32);
            let y2 = cmd.y + (cmd_h as f32 / self.scale_factor as f32);
            let u1 = 0.0;
            let v1 = current_y as f32 / atlas_h as f32;
            let u2 = cmd_w as f32 / atlas_w as f32;
            let v2 = (current_y + cmd_h) as f32 / atlas_h as f32;

            vertices.push(TextVertex { position: [x1, y1], texcoord: [u1, v1], color: cmd.color });
            vertices.push(TextVertex { position: [x2, y1], texcoord: [u2, v1], color: cmd.color });
            vertices.push(TextVertex { position: [x2, y2], texcoord: [u2, v2], color: cmd.color });
            vertices.push(TextVertex { position: [x1, y2], texcoord: [u1, v2], color: cmd.color });
            indices.push(base);
            indices.push(base + 1);
            indices.push(base + 2);
            indices.push(base);
            indices.push(base + 2);
            indices.push(base + 3);

            current_y += cmd_h;
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
        // bytes_per_row must be aligned to COPY_BYTES_PER_ROW_ALIGNMENT (256)
        // Use the same aligned stride from allocation
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

    /// Submit all queued text to the GPU.
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
