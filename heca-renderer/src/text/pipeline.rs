//! **The graphics side of text**: the GPU setup, the vertex layout, clipping a draw to the screen,
//! and drawing a frame's queued text.

use cosmic_text::SwashCache;
use wgpu::util::DeviceExt;

use super::TextRenderer;
use crate::clip::combine_clip;
use crate::composite::content_clip_stencil_state;
use crate::font;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct TextVertex {
    pub(super) position: [f32; 2],
    pub(super) texcoord: [f32; 2],
    pub(super) color: [f32; 4],
    /// `0.0` = a normal glyph (occludes what is behind it); `1.0` = an additive
    /// halo tap (adds light, writes no alpha). Under premultiplied-alpha blending
    /// a fragment with `a == 0` and `rgb > 0` is exactly additive — the same trick
    /// `grid.wgsl` uses to let a rect's glow add light without occluding.
    pub(super) additive: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct Uniforms {
    pub(super) screen_size: [f32; 2],
    pub(super) _pad: [f32; 2],
}

/// One label's draw range: its glyph quads occupy `[first_index, first_index +
/// index_count)` of the frame's index buffer, scissored to `clip`.
pub(super) struct DrawRange {
    pub(super) clip: Option<[f32; 4]>,
    pub(super) first_index: u32,
    pub(super) index_count: u32,
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

/// The vertex buffer's size: per-glyph quads (one per glyph, not one per label) need room, so it
/// is sized generously.
const VERTEX_BUFFER_BYTES: u64 = 4 * 1024 * 1024;
/// The index buffer's size. Indices are `u32`: a frame can exceed 65 536 vertices.
const INDEX_BUFFER_BYTES: u64 = 1024 * 1024;

impl TextVertex {
    /// Where each field sits in a vertex, in the shader's locations: position, texcoord, color,
    /// additive.
    pub(super) const ATTRIBUTES: [wgpu::VertexAttribute; 4] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4, 3 => Float32];
}

/// What every text draw binds: the screen-size uniform, the glyph atlas and its sampler.
fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
    })
}

/// The one bind group bound for all text draws: the uniform, the shared glyph atlas, and a linear
/// sampler that clamps at the atlas edges.
fn atlas_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniforms: &wgpu::Buffer,
    atlas: &crate::atlas::Atlas,
) -> wgpu::BindGroup {
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("text_sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    });
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("text_atlas_bind_group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
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
    })
}

/// **The text pipeline**, plain or — given a stencil state — testing the rounded content-clip mask.
/// One definition for both, so the two cannot drift apart in anything but the stencil.
fn text_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    target_format: wgpu::TextureFormat,
    stencil: Option<wgpu::DepthStencilState>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(if stencil.is_some() {
            "text_stencil_pipeline"
        } else {
            "text_pipeline"
        }),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<TextVertex>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &TextVertex::ATTRIBUTES,
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
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
        depth_stencil: stencil,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

/// A buffer of `size` bytes the frame's geometry is written into.
fn geometry_buffer(
    device: &wgpu::Device,
    label: &str,
    size: u64,
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

impl TextRenderer {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("text_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../text.wgsl").into()),
        });
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("text_uniforms"),
            contents: bytemuck::cast_slice(&[Uniforms {
                screen_size: [1.0, 1.0],
                _pad: [0.0; 2],
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group_layout = bind_group_layout(device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("text_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = text_pipeline(device, &pipeline_layout, &shader, target_format, None);
        // Tests the rounded content-clip mask (`Equal` to ref 1, set per pass). Depth disabled.
        let stencil_pipeline = text_pipeline(
            device,
            &pipeline_layout,
            &shader,
            target_format,
            Some(content_clip_stencil_state()),
        );
        let atlas = crate::atlas::Atlas::new(device);
        let atlas_bind_group =
            atlas_bind_group(device, &bind_group_layout, &uniform_buffer, &atlas);

        Self {
            glow_alpha_scale: 0.0,
            font_system: font::embedded_font_system(),
            swash_cache: SwashCache::new(),
            pipeline,
            stencil_pipeline,
            vertex_buffer: geometry_buffer(
                device,
                "text_vertex_buffer",
                VERTEX_BUFFER_BYTES,
                wgpu::BufferUsages::VERTEX,
            ),
            index_buffer: geometry_buffer(
                device,
                "text_index_buffer",
                INDEX_BUFFER_BYTES,
                wgpu::BufferUsages::INDEX,
            ),
            uniform_buffer,
            scale_factor: 1.0,
            target_size: [1, 1],
            current_clip: None,
            damage: None,
            frame_vtx: 0,
            passes: 0,
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

        self.passes += 1;
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
