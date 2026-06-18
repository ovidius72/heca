//! GPU renderer for Grid UI primitives: SDF rounded rectangles with border and
//! additive neon glow. Backs `heca-grid-ui`'s `Rect`/`Brackets` draw commands.

use wgpu::util::DeviceExt;

use crate::clip::combine_clip;
use crate::composite::STENCIL_FORMAT;

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
    shadow: [f32; 4],
    shadow_radius: f32,
    shadow_offset: [f32; 2],
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
    /// Drop-shadow color (premultiply-friendly straight color + alpha).
    pub shadow: [f32; 4],
    /// Shadow blur/falloff radius (logical px); `0` = no shadow.
    pub shadow_radius: f32,
    /// Shadow offset (logical px): `[dx, dy]`, positive = right/down.
    pub shadow_offset: [f32; 2],
}

/// Renders SDF rounded rects with glow. Mirrors `PrimitiveRenderer`'s buffer
/// staging pattern so it slots into the same render loop.
pub struct GridRenderer {
    pipeline: wgpu::RenderPipeline,
    /// Stencil-write variant: same vertex layout + SDF shader, `fs_stencil`
    /// fragment (keep inside the rounded rect), `StencilOp::Replace`, no color
    /// targets. Backs [`render_stencil`](Self::render_stencil).
    stencil_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    vertices: Vec<GridVertex>,
    indices: Vec<u32>,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    /// Logical→physical scale, for converting clip rects to scissor pixels.
    scale_factor: f32,
    /// Physical framebuffer size `[w, h]`, used to clamp scissor rects in-bounds.
    target_size: [u32; 2],
    /// Active clip changes: `(index offset at the change, clip rect or `None`)`.
    /// `render` turns these into scissored draw spans. Logical-pixel rects.
    clip_marks: Vec<(u32, Option<[f32; 4]>)>,
    /// The clip currently in effect for new `draw` calls.
    current_clip: Option<[f32; 4]>,
    /// Frame-level damage region (logical px): when set, the whole pass is also
    /// scissored to this, so a damage-region redraw only touches changed pixels.
    /// `None` = no damage limit (full frame).
    damage: Option<[f32; 4]>,
    /// Vertices/indices already written to the persistent buffers this frame. Each
    /// render pass appends at its own offset (via `queue.write_buffer`) instead of
    /// allocating a fresh staging buffer — the per-frame allocation that caused
    /// frame-time spikes. Reset by [`begin_frame`](GridRenderer::begin_frame).
    frame_vtx: u32,
    frame_idx: u32,
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
            10 => Float32x4, // shadow
            11 => Float32,   // shadow_radius
            12 => Float32x2, // shadow_offset
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

        // Stencil-write pipeline: same vertex layout + SDF shader, but a fragment
        // that keeps only fragments INSIDE the rounded rect (`fs_stencil`) and a
        // stencil state that `Replace`-s the stencil with the pass reference. No
        // color targets — the pass attaches only the stencil buffer. Used by
        // `render_stencil` to write the rounded content-clip mask.
        let stencil_face = wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Always,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: wgpu::StencilOperation::Replace,
        };
        let stencil_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("grid_stencil_pipeline"),
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
                entry_point: Some("fs_stencil"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: STENCIL_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState {
                    front: stencil_face,
                    back: stencil_face,
                    read_mask: 0xFFFFFFFF,
                    write_mask: 0xFFFFFFFF,
                },
                bias: wgpu::DepthBiasState::default(),
            }),
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
            stencil_pipeline,
            vertex_buffer,
            index_buffer,
            vertices: Vec::new(),
            indices: Vec::new(),
            bind_group,
            uniform_buffer,
            scale_factor: 1.0,
            target_size: [1, 1],
            clip_marks: Vec::new(),
            current_clip: None,
            damage: None,
            frame_vtx: 0,
            frame_idx: 0,
        }
    }

    /// Reset the per-frame buffer write offsets. Call once at the start of each
    /// frame, before any `render` passes, so the frame's passes append to the
    /// persistent buffers instead of overwriting one another.
    pub fn begin_frame(&mut self) {
        self.frame_vtx = 0;
        self.frame_idx = 0;
    }

    /// Set the frame-level damage region (logical px) the whole pass is scissored
    /// to, or `None` for a full-frame render. Set once per frame before `render`.
    pub fn set_damage(&mut self, damage: Option<[f32; 4]>) {
        self.damage = damage;
    }

    /// Logical→physical scale factor (HiDPI). Used to map clip rects to scissor px.
    pub fn set_scale_factor(&mut self, scale: f64) {
        self.scale_factor = scale as f32;
    }

    /// Physical framebuffer size in pixels; scissor rects are clamped to it.
    pub fn set_target_size(&mut self, width: u32, height: u32) {
        self.target_size = [width.max(1), height.max(1)];
    }

    /// Set the clip rect (logical px, `[x, y, w, h]`) applied to subsequent
    /// [`draw`](Self::draw) calls, or `None` to clear it. A no-op if unchanged.
    pub fn set_clip(&mut self, clip: Option<[f32; 4]>) {
        if clip != self.current_clip {
            self.clip_marks.push((self.indices.len() as u32, clip));
            self.current_clip = clip;
        }
    }

    /// Convert a logical clip rect to a physical scissor rect clamped to the
    /// framebuffer: `(x, y, w, h)`. A zero `w`/`h` means "fully clipped".
    fn scissor_px(&self, c: [f32; 4]) -> (u32, u32, u32, u32) {
        let s = self.scale_factor;
        let (fw, fh) = (self.target_size[0] as f32, self.target_size[1] as f32);
        let x0 = (c[0] * s).clamp(0.0, fw);
        let y0 = (c[1] * s).clamp(0.0, fh);
        let x1 = ((c[0] + c[2]) * s).clamp(0.0, fw);
        let y1 = ((c[1] + c[3]) * s).clamp(0.0, fh);
        (
            x0 as u32,
            y0 as u32,
            (x1 - x0).max(0.0) as u32,
            (y1 - y0).max(0.0) as u32,
        )
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
        // Expand the quad to contain whichever halo reaches furthest: the glow, or
        // the (offset) drop shadow's blur — so neither falloff is clipped.
        let shadow_reach = r.shadow_radius.max(0.0)
            + r.shadow_offset[0].abs().max(r.shadow_offset[1].abs());
        let margin = r.glow_radius.max(0.0).max(shadow_reach).max(r.border_width) + 2.0;
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
            shadow: r.shadow,
            shadow_radius: r.shadow_radius,
            shadow_offset: r.shadow_offset,
        };

        let base = self.vertices.len() as u32;
        self.vertices.push(v([x0, y0]));
        self.vertices.push(v([x1, y0]));
        self.vertices.push(v([x1, y1]));
        self.vertices.push(v([x0, y1]));
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Submit all queued primitives. Appends this pass's geometry to the persistent
    /// vertex/index buffers at the running frame offset (no per-frame staging
    /// allocation); call [`begin_frame`](GridRenderer::begin_frame) once per frame.
    pub fn render(
        &mut self,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if self.vertices.is_empty() {
            return;
        }

        let vertex_data: &[u8] = bytemuck::cast_slice(&self.vertices);
        let index_data: &[u8] = bytemuck::cast_slice(&self.indices);
        let v_off = self.frame_vtx as u64 * std::mem::size_of::<GridVertex>() as u64;
        let i_off = self.frame_idx as u64 * std::mem::size_of::<u32>() as u64;
        // Append to the persistent buffers via the queue's reusable staging belt —
        // distinct offsets per pass so a later pass doesn't clobber an earlier one.
        queue.write_buffer(&self.vertex_buffer, v_off, vertex_data);
        queue.write_buffer(&self.index_buffer, i_off, index_data);

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

        // Draw in clip spans: the leading span (before the first clip mark) is
        // unclipped, then each mark opens a new scissored span. With no clipping in
        // use this is a single full-framebuffer span — one draw, as before. Indices
        // and vertices live at this frame's running offset, so draws reference them
        // via `first_index` (idx_base + span) and `base_vertex`.
        let total = self.indices.len() as u32;
        let idx_base = self.frame_idx;
        let base_vertex = self.frame_vtx as i32;
        let tail: (u32, Option<[f32; 4]>) = (total, None);
        let mut start = 0u32;
        let mut clip: Option<[f32; 4]> = None;
        for &(at, next_clip) in self.clip_marks.iter().chain(std::iter::once(&tail)) {
            if at > start {
                let draw = match combine_clip(self.damage, clip) {
                    None => {
                        rpass.set_scissor_rect(0, 0, self.target_size[0], self.target_size[1]);
                        true
                    }
                    Some(c) => {
                        let (x, y, w, h) = self.scissor_px(c);
                        if w == 0 || h == 0 {
                            false // span fully outside the damage/clip — skip it
                        } else {
                            rpass.set_scissor_rect(x, y, w, h);
                            true
                        }
                    }
                };
                if draw {
                    rpass.draw_indexed((idx_base + start)..(idx_base + at), base_vertex, 0..1);
                }
            }
            start = at;
            clip = next_clip;
        }

        self.frame_vtx += self.vertices.len() as u32;
        self.frame_idx += self.indices.len() as u32;
        self.vertices.clear();
        self.indices.clear();
        self.clip_marks.clear();
        self.current_clip = None;
    }

    /// Write the queued rounded rects to the stencil buffer as a rounded
    /// content-clip mask, then clear the queue. The pass clears the stencil to 0
    /// and `Replace`-writes 1 inside each rounded rect (via `fs_stencil`, which
    /// discards outside). Content renderers then attach `stencil_view` and test
    /// `Equal` to ref 1 so they only draw inside the rounded shape. Call once per
    /// frame after queueing the panes' inner rounded rects with [`draw`](Self::draw),
    /// before the content render passes. Appends at the running frame offset like
    /// [`render`](Self::render), so it composes with the color passes.
    pub fn render_stencil(
        &mut self,
        queue: &wgpu::Queue,
        stencil_view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if self.vertices.is_empty() {
            return;
        }

        let vertex_data: &[u8] = bytemuck::cast_slice(&self.vertices);
        let index_data: &[u8] = bytemuck::cast_slice(&self.indices);
        let v_off = self.frame_vtx as u64 * std::mem::size_of::<GridVertex>() as u64;
        let i_off = self.frame_idx as u64 * std::mem::size_of::<u32>() as u64;
        queue.write_buffer(&self.vertex_buffer, v_off, vertex_data);
        queue.write_buffer(&self.index_buffer, i_off, index_data);

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("grid_stencil_pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: stencil_view,
                depth_ops: None,
                stencil_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0u32),
                    store: wgpu::StoreOp::Store,
                }),
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        rpass.set_pipeline(&self.stencil_pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        rpass.set_stencil_reference(1);
        // Draw the whole mask unscissored — `fs_stencil` shapes it via the SDF.
        // Clip marks are ignored here (the queued rects ARE the rounded masks,
        // not scissored color spans).
        let idx_base = self.frame_idx;
        let base_vertex = self.frame_vtx as i32;
        rpass.draw_indexed(idx_base..idx_base + self.indices.len() as u32, base_vertex, 0..1);

        self.frame_vtx += self.vertices.len() as u32;
        self.frame_idx += self.indices.len() as u32;
        self.vertices.clear();
        self.indices.clear();
        self.clip_marks.clear();
        self.current_clip = None;
    }
}
