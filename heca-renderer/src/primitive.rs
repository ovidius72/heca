use wgpu::util::DeviceExt;

use crate::composite::content_clip_stencil_state;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 4],
}

/// GPU renderer for primitive shapes: rectangles, borders.
pub struct PrimitiveRenderer {
    pipeline: wgpu::RenderPipeline,
    /// Stencil-test variant: same as `pipeline` but tests `Equal` against the
    /// rounded content-clip mask. Used when `render_clipped` is given a stencil.
    stencil_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    screen_size: [f32; 2],
    _pad: [f32; 2],
}

impl PrimitiveRenderer {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("primitive_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("primitive.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("primitive_uniforms"),
            contents: bytemuck::cast_slice(&[Uniforms {
                screen_size: [1.0, 1.0],
                _pad: [0.0; 2],
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("primitive_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("primitive_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("primitive_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("primitive_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
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
            label: Some("primitive_stencil_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
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

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("primitive_vertex_buffer"),
            size: 1024 * 1024, // 1MB
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("primitive_index_buffer"),
            size: 512 * 1024, // 512KB
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            stencil_pipeline,
            vertex_buffer,
            index_buffer,
            vertices: Vec::new(),
            indices: Vec::new(),
            bind_group,
            uniform_buffer,
        }
    }

    pub fn set_screen_size(&mut self, queue: &wgpu::Queue, width: f32, height: f32) {
        let uniforms = Uniforms {
            screen_size: [width, height],
            _pad: [0.0; 2],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    /// Queue a filled rectangle.
    pub fn draw_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        let base = self.vertices.len() as u32;
        self.vertices.push(Vertex {
            position: [x, y],
            color,
        });
        self.vertices.push(Vertex {
            position: [x + w, y],
            color,
        });
        self.vertices.push(Vertex {
            position: [x + w, y + h],
            color,
        });
        self.vertices.push(Vertex {
            position: [x, y + h],
            color,
        });
        self.indices.push(base);
        self.indices.push(base + 1);
        self.indices.push(base + 2);
        self.indices.push(base);
        self.indices.push(base + 2);
        self.indices.push(base + 3);
    }

    /// Queue a rectangle border (outline).
    pub fn draw_border(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4], width: f32) {
        // Top
        self.draw_rect(x, y, w, width, color);
        // Bottom
        self.draw_rect(x, y + h - width, w, width, color);
        // Left
        self.draw_rect(x, y + width, width, h - 2.0 * width, color);
        // Right
        self.draw_rect(x + w - width, y + width, width, h - 2.0 * width, color);
    }

    /// Queue an outline that sits outside the given rectangle and does not
    /// consume any of its interior space.
    pub fn draw_outline(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4], width: f32) {
        self.draw_border(
            x - width,
            y - width,
            w + width * 2.0,
            h + width * 2.0,
            color,
            width,
        );
    }

    /// Queue a filled rectangle with a border.
    /// Draw a filled rectangle with a border.
    ///
    /// Note: `radius` is currently ignored. True rounded corners (SDF or
    /// geometry-based) are planned for a future phase. The function name is
    /// kept for API stability but the output is currently sharp-cornered.
    #[expect(clippy::too_many_arguments, reason = "Renderer primitive API keeps rectangle geometry and fill/border styling explicit at call sites.")]
    pub fn draw_rounded_rect(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        fill: [f32; 4],
        border: [f32; 4],
        border_width: f32,
        _radius: f32,
    ) {
        self.draw_rect(x, y, w, h, fill);
        self.draw_border(x, y, w, h, border, border_width);
    }

    /// Queue a filled triangle.
    pub fn draw_triangle(
        &mut self,
        a: [f32; 2],
        b: [f32; 2],
        c: [f32; 2],
        color: [f32; 4],
    ) {
        let base = self.vertices.len() as u32;
        self.vertices.push(Vertex { position: a, color });
        self.vertices.push(Vertex { position: b, color });
        self.vertices.push(Vertex { position: c, color });
        self.indices.push(base);
        self.indices.push(base + 1);
        self.indices.push(base + 2);
    }

    /// Submit all queued primitives to the GPU.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        self.render_clipped(device, view, encoder, None, None);
    }

    /// Submit all queued primitives to the GPU with an optional scissor clip.
    /// When `stencil` is `Some`, the pass tests against the rounded content-clip
    /// mask (stencil `Equal` to ref 1) so primitives only draw inside the pane's
    /// rounded shape — used for terminal surface/cell/selection/cursor fills.
    pub fn render_clipped(
        &mut self,
        device: &wgpu::Device,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        clip_rect: Option<(u32, u32, u32, u32)>,
        stencil: Option<&wgpu::TextureView>,
    ) {
        if self.vertices.is_empty() {
            return;
        }

        let vertex_data = bytemuck::cast_slice(&self.vertices);
        let index_data = bytemuck::cast_slice(&self.indices);

        let staging_vertex = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("primitive_vertex_staging"),
            contents: vertex_data,
            usage: wgpu::BufferUsages::COPY_SRC,
        });

        let staging_index = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("primitive_index_staging"),
            contents: index_data,
            usage: wgpu::BufferUsages::COPY_SRC,
        });

        encoder.copy_buffer_to_buffer(
            &staging_vertex,
            0,
            &self.vertex_buffer,
            0,
            vertex_data.len() as u64,
        );
        encoder.copy_buffer_to_buffer(
            &staging_index,
            0,
            &self.index_buffer,
            0,
            index_data.len() as u64,
        );

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("primitive_render_pass"),
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
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        if stencil.is_some() {
            rpass.set_stencil_reference(1);
        }
        if let Some((x, y, w, h)) = clip_rect {
            rpass.set_scissor_rect(x, y, w, h);
        }
        rpass.draw_indexed(0..self.indices.len() as u32, 0, 0..1);

        self.vertices.clear();
        self.indices.clear();
    }
}
