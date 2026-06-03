//! GPU renderer for Grid UI primitives: SDF rounded rectangles with border and
//! additive neon glow. Backs `heca-grid-ui`'s `Rect`/`Brackets` draw commands.

use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GridVertex {
    position: [f32; 2],
    center: [f32; 2],
    half_size: [f32; 2],
    radius: f32,
    fill: [f32; 4],
    border: [f32; 4],
    border_width: f32,
    glow: [f32; 4],
    glow_radius: f32,
    glow_intensity: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    screen_size: [f32; 2],
    _pad: [f32; 2],
}

/// Parameters for one glowing rounded rect. All coordinates are logical pixels.
#[derive(Clone, Copy, Debug)]
pub struct GlowRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub fill: [f32; 4],
    pub border: [f32; 4],
    pub border_width: f32,
    pub radius: f32,
    pub glow: [f32; 4],
    pub glow_radius: f32,
    pub glow_intensity: f32,
}

/// Renders SDF rounded rects with glow. Mirrors `PrimitiveRenderer`'s buffer
/// staging pattern so it slots into the same render loop.
pub struct GridRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    vertices: Vec<GridVertex>,
    indices: Vec<u32>,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
}

impl GridRenderer {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("grid_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("grid.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("grid_uniforms"),
            contents: bytemuck::cast_slice(&[Uniforms {
                screen_size: [1.0, 1.0],
                _pad: [0.0; 2],
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("grid_bind_group_layout"),
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
            label: Some("grid_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("grid_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Float32x2 ×3, Float32, Float32x4 ×2, Float32, Float32x4, Float32 ×2.
        let attributes = wgpu::vertex_attr_array![
            0 => Float32x2, // position
            1 => Float32x2, // center
            2 => Float32x2, // half_size
            3 => Float32,   // radius
            4 => Float32x4, // fill
            5 => Float32x4, // border
            6 => Float32,   // border_width
            7 => Float32x4, // glow
            8 => Float32,   // glow_radius
            9 => Float32,   // glow_intensity
        ];

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("grid_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GridVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    // Premultiplied alpha: fills composite normally, while the
                    // glow (alpha 0, color > 0) adds light without occluding.
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("grid_vertex_buffer"),
            size: 4 * 1024 * 1024,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("grid_index_buffer"),
            size: 1024 * 1024,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
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

    /// Queue one glowing rounded rect. The quad is expanded to contain the glow
    /// halo so the falloff isn't clipped.
    pub fn draw(&mut self, r: GlowRect) {
        let margin = r.glow_radius.max(0.0) + 2.0;
        let (x0, y0) = (r.x - margin, r.y - margin);
        let (x1, y1) = (r.x + r.w + margin, r.y + r.h + margin);
        let center = [r.x + r.w * 0.5, r.y + r.h * 0.5];
        let half = [r.w * 0.5, r.h * 0.5];

        let v = |pos: [f32; 2]| GridVertex {
            position: pos,
            center,
            half_size: half,
            radius: r.radius,
            fill: r.fill,
            border: r.border,
            border_width: r.border_width,
            glow: r.glow,
            glow_radius: r.glow_radius,
            glow_intensity: r.glow_intensity,
        };

        let base = self.vertices.len() as u32;
        self.vertices.push(v([x0, y0]));
        self.vertices.push(v([x1, y0]));
        self.vertices.push(v([x1, y1]));
        self.vertices.push(v([x0, y1]));
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Submit all queued primitives.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if self.vertices.is_empty() {
            return;
        }

        let vertex_data = bytemuck::cast_slice(&self.vertices);
        let index_data = bytemuck::cast_slice(&self.indices);

        let staging_vertex = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("grid_vertex_staging"),
            contents: vertex_data,
            usage: wgpu::BufferUsages::COPY_SRC,
        });
        let staging_index = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("grid_index_staging"),
            contents: index_data,
            usage: wgpu::BufferUsages::COPY_SRC,
        });

        encoder.copy_buffer_to_buffer(&staging_vertex, 0, &self.vertex_buffer, 0, vertex_data.len() as u64);
        encoder.copy_buffer_to_buffer(&staging_index, 0, &self.index_buffer, 0, index_data.len() as u64);

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("grid_render_pass"),
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
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        rpass.draw_indexed(0..self.indices.len() as u32, 0, 0..1);

        self.vertices.clear();
        self.indices.clear();
    }
}
