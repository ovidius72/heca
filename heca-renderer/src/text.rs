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

/// A GPU-ready text label: texture + quad.
struct TextLabel {
    bind_group: wgpu::BindGroup,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: [f32; 4],
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

    /// Render all queued text. Returns (vertices, indices, labels) for drawing.
    fn build_labels(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> (Vec<TextVertex>, Vec<u16>, Vec<TextLabel>) {
        if self.commands.is_empty() {
            return (Vec::new(), Vec::new(), Vec::new());
        }

        let scale = self.scale_factor as f32;
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut labels = Vec::new();
        let mut base = 0u16;

        for cmd in &self.commands {
            // 1. Shape text
            let scaled_size = cmd.font_size * scale;
            let metrics = Metrics::new(scaled_size, scaled_size * 1.2);
            let mut buffer = Buffer::new(&mut self.font_system, metrics);
            buffer.set_size(&mut self.font_system, Some(4096.0), Some(4096.0));
            let attrs = Attrs::new().family(Family::Name(&self.font_family));
            buffer.set_text(&mut self.font_system, &cmd.text, &attrs, Shaping::Advanced);
            buffer.shape_until_scroll(&mut self.font_system, false);

            // 2. Measure exact bounds
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
                continue;
            }

            let content_w = (max_x - min_x) as u32;
            let content_h = (max_y - min_y) as u32;
            if content_w == 0 || content_h == 0 {
                continue;
            }

            // 3. Render into CPU buffer using Buffer::draw (SAME buffer!)
            // wgpu requires bytes_per_row to be a multiple of 256
            let align = |v: u32| ((v + 255) / 256) * 256;
            let stride = align(content_w);
            let mut pixels = vec![0u8; (stride * content_h) as usize];
            let w_local = content_w;
            let h_local = content_h;
            let min_x_local = min_x;
            let min_y_local = min_y;

            buffer.draw(
                &mut self.font_system,
                &mut self.swash_cache,
                CosmicColor::rgb(0xFF, 0xFF, 0xFF),
                |x: i32, y: i32, _w: u32, _h: u32, color: CosmicColor| {
                    let a = color.a();
                    if a == 0 {
                        return;
                    }
                    let px = (x - min_x_local) as u32;
                    let py = (y - min_y_local) as u32;
                    if px < w_local && py < h_local {
                        let idx = (py * stride + px) as usize;
                        pixels[idx] = pixels[idx].saturating_add(a);
                    }
                },
            );

            // DEBUG: check if any pixel is non-zero
            let non_zero = pixels.iter().filter(|&&v| v > 0).count();
            eprintln!("[heca-text] '{}' -> {}x{} pixels, {} non-zero", cmd.text, content_w, content_h, non_zero);

            // 4. Upload as individual texture
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

            // 5. Build quad
            let screen_x = cmd.x + min_x as f32 / scale;
            let screen_y = cmd.y + min_y as f32 / scale;
            let screen_w = content_w as f32 / scale;
            let screen_h = content_h as f32 / scale;

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

            labels.push(TextLabel {
                bind_group,
                x: screen_x,
                y: screen_y,
                w: screen_w,
                h: screen_h,
                color: cmd.color,
            });

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
        if self.commands.is_empty() {
            return;
        }

        let (vertices, indices, labels) = self.build_labels(device, queue);

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
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        // Draw each label with its own bind group
        for (i, label) in labels.iter().enumerate() {
            rpass.set_bind_group(0, &label.bind_group, &[]);
            let start = i as u32 * 6;
            rpass.draw_indexed(start..start + 6, 0, 0..1);
        }

        self.commands.clear();
    }
}
