//! Inline terminal-image rendering: blits decoded images (Sixel / iTerm2 /
//! Kitty graphics, all captured uniformly by the backend) as textured quads into
//! the terminal scratch layer.
//!
//! Each unique source image is uploaded to a GPU texture once, cached by the
//! backend's content-hash-derived `image_id`, and reused across frames. Per
//! frame the terminal renderer queues one quad per [`GraphicsPlacement`]; the
//! quads are flushed in two layers so `z < 0` images sit under the glyphs and
//! `z >= 0` images sit over them.

use std::collections::HashMap;

use heca_core::backend::{GraphicsPlacement, TerminalImage};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    screen_size: [f32; 2],
}

/// Which side of the glyphs a flush draws. Image placements carry a `z_index`;
/// negative values render under the text, non-negative over it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ImageLayer {
    UnderText,
    OverText,
}

/// One queued image quad in logical pixel coordinates.
#[derive(Copy, Clone, Debug)]
struct QueuedQuad {
    image_id: u64,
    z_index: i32,
    /// Destination rect `[x, y, w, h]` in logical pixels.
    dst: [f32; 4],
    /// Source texture coordinates at the top-left and bottom-right corners.
    src_top_left: [f32; 2],
    src_bottom_right: [f32; 2],
}

struct CachedImage {
    /// The GPU texture holding the currently-uploaded frame. Retained so an
    /// animated image can re-upload the next frame in place (all frames share the
    /// image's `width`/`height`).
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    /// Generation of the last frame that referenced this image; drives eviction.
    last_used: u64,
    /// Playback state for an animated image (`None` for a static one).
    anim: Option<AnimPlayback>,
}

/// Wall-clock playback state for one animated inline image.
struct AnimPlayback {
    /// When the animation was first uploaded; the shown frame derives from
    /// `now - start` looped over the total duration.
    start: std::time::Instant,
    /// Which frame index is currently written into the texture.
    uploaded_frame: usize,
}

/// GPU renderer for inline terminal images.
pub struct ImageRenderer {
    pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    cache: HashMap<u64, CachedImage>,
    queued: Vec<QueuedQuad>,
    generation: u64,
}

impl ImageRenderer {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("image_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("image.wgsl").into()),
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("image_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("image_uniforms"),
            contents: bytemuck::cast_slice(&[Uniforms {
                screen_size: [1.0, 1.0],
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("image_uniform_layout"),
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

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("image_uniform_bind_group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("image_texture_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("image_pipeline_layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("image_pipeline"),
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
                            format: wgpu::VertexFormat::Float32x2,
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
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            sampler,
            uniform_buffer,
            uniform_bind_group,
            texture_bind_group_layout,
            cache: HashMap::new(),
            queued: Vec::new(),
            generation: 0,
        }
    }

    /// Logical coordinate space the queued quad positions map onto.
    pub fn set_screen_size(&mut self, queue: &wgpu::Queue, width: f32, height: f32) {
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[Uniforms {
                screen_size: [width.max(1.0), height.max(1.0)],
            }]),
        );
    }

    /// Begin a new frame: bump the generation so [`Self::evict_unused`] can drop
    /// images not referenced since.
    pub fn begin_frame(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    /// Ensure every image in `images` has a GPU texture, uploading new ones, and
    /// mark them all used this generation. Idempotent per id across frames.
    ///
    /// For animated images, advances the playback clock: when the frame shown at
    /// "now" differs from the one in the texture, the new frame is re-uploaded in
    /// place. Returns `true` when any animated image advanced a frame this call,
    /// so the caller can damage its rows and keep requesting frames.
    pub fn upload_images(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        images: &[TerminalImage],
    ) -> bool {
        let now = std::time::Instant::now();
        let mut advanced = false;
        for image in images {
            match self.cache.get_mut(&image.id) {
                Some(cached) => {
                    cached.last_used = self.generation;
                    if let Some(anim) = cached.anim.as_mut() {
                        let desired =
                            image.frame_index_at(now.saturating_duration_since(anim.start));
                        if desired != anim.uploaded_frame
                            && let Some(frame) = image.frames.get(desired)
                        {
                            write_image_frame(queue, &cached.texture, image, &frame.rgba);
                            anim.uploaded_frame = desired;
                            advanced = true;
                        }
                    }
                }
                None => {
                    let (texture, bind_group) = self.upload_one(device, queue, image);
                    let anim = image.is_animated().then_some(AnimPlayback {
                        start: now,
                        uploaded_frame: 0,
                    });
                    self.cache.insert(
                        image.id,
                        CachedImage {
                            texture,
                            bind_group,
                            last_used: self.generation,
                            anim,
                        },
                    );
                }
            }
        }
        advanced
    }

    fn upload_one(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        image: &TerminalImage,
    ) -> (wgpu::Texture, wgpu::BindGroup) {
        let size = wgpu::Extent3d {
            width: image.width.max(1),
            height: image.height.max(1),
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("terminal_inline_image"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        // The first frame is always present (frames is non-empty by construction).
        if let Some(frame) = image.frames.first() {
            write_image_frame(queue, &texture, image, &frame.rgba);
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terminal_inline_image_bind_group"),
            layout: &self.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        (texture, bind_group)
    }

    /// Drop cached textures not referenced in the last `max_age` generations.
    pub fn evict_unused(&mut self, max_age: u64) {
        let cutoff = self.generation.wrapping_sub(max_age);
        let current = self.generation;
        self.cache.retain(|_, cached| {
            // Keep anything used this generation or within `max_age` of it,
            // accounting for the (astronomically rare) wrap.
            let age = current.wrapping_sub(cached.last_used);
            cached.last_used == current || age <= max_age && cached.last_used != cutoff
        });
    }

    /// Queue one placement for this frame, mapping its cell rect to a logical
    /// pixel rect anchored at `(origin_x, origin_y)`.
    pub fn queue_placement(
        &mut self,
        placement: &GraphicsPlacement,
        origin_x: f32,
        origin_y: f32,
        cell_w: f32,
        cell_h: f32,
    ) {
        self.queued.push(QueuedQuad {
            image_id: placement.image_id,
            z_index: placement.z_index,
            dst: placement_dst_rect(placement, origin_x, origin_y, cell_w, cell_h),
            src_top_left: placement.src_top_left,
            src_bottom_right: placement.src_bottom_right,
        });
    }

    /// Flush the queued quads for one layer into `view`. Quads are grouped by
    /// image so each texture binds once. The matching queue entries are consumed;
    /// call with `UnderText` before the glyph pass and `OverText` after it.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        layer: ImageLayer,
    ) {
        let belongs = |z: i32| match layer {
            ImageLayer::UnderText => z < 0,
            ImageLayer::OverText => z >= 0,
        };
        if !self.queued.iter().any(|q| belongs(q.z_index)) {
            // Still drop this layer's (empty) entries so a later layer flush is clean.
            self.queued.retain(|q| !belongs(q.z_index));
            return;
        }

        // Group quads by image, preserving first-seen order for stable layering.
        let mut order: Vec<u64> = Vec::new();
        let mut by_image: HashMap<u64, Vec<Vertex>> = HashMap::new();
        for quad in self.queued.iter().filter(|q| belongs(q.z_index)) {
            if !self.cache.contains_key(&quad.image_id) {
                continue; // texture not uploaded (decode failed or evicted)
            }
            let verts = by_image.entry(quad.image_id).or_insert_with(|| {
                order.push(quad.image_id);
                Vec::new()
            });
            verts.extend_from_slice(&quad_vertices(quad));
        }
        self.queued.retain(|q| !belongs(q.z_index));

        if order.is_empty() {
            return;
        }

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("image_render_pass"),
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
        rpass.set_bind_group(0, &self.uniform_bind_group, &[]);

        for image_id in order {
            let verts = &by_image[&image_id];
            let indices = quad_indices(verts.len() / 4);
            let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("image_vertex_buffer"),
                contents: bytemuck::cast_slice(verts),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("image_index_buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            let bind_group = &self.cache[&image_id].bind_group;
            rpass.set_bind_group(1, bind_group, &[]);
            rpass.set_vertex_buffer(0, vbuf.slice(..));
            rpass.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint32);
            rpass.draw_indexed(0..indices.len() as u32, 0, 0..1);
        }
    }
}

/// Write one frame's RGBA pixels into `texture` (all frames share the image's
/// `width`/`height`, so the texture size never changes across a frame advance).
fn write_image_frame(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    image: &TerminalImage,
    rgba: &[u8],
) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(image.width * 4),
            rows_per_image: Some(image.height),
        },
        wgpu::Extent3d {
            width: image.width.max(1),
            height: image.height.max(1),
            depth_or_array_layers: 1,
        },
    );
}

/// Map a placement's cell rect to a logical pixel rect `[x, y, w, h]`.
pub fn placement_dst_rect(
    placement: &GraphicsPlacement,
    origin_x: f32,
    origin_y: f32,
    cell_w: f32,
    cell_h: f32,
) -> [f32; 4] {
    [
        origin_x + placement.col as f32 * cell_w,
        origin_y + placement.row as f32 * cell_h,
        placement.cols as f32 * cell_w,
        placement.rows as f32 * cell_h,
    ]
}

/// Four corner vertices (two triangles via [`quad_indices`]) for one quad.
fn quad_vertices(quad: &QueuedQuad) -> [Vertex; 4] {
    let [x, y, w, h] = quad.dst;
    let [u0, v0] = quad.src_top_left;
    let [u1, v1] = quad.src_bottom_right;
    [
        Vertex {
            position: [x, y],
            uv: [u0, v0],
        },
        Vertex {
            position: [x + w, y],
            uv: [u1, v0],
        },
        Vertex {
            position: [x + w, y + h],
            uv: [u1, v1],
        },
        Vertex {
            position: [x, y + h],
            uv: [u0, v1],
        },
    ]
}

/// Index list (`0,1,2, 0,2,3` per quad) for `quad_count` quads.
fn quad_indices(quad_count: usize) -> Vec<u32> {
    let mut indices = Vec::with_capacity(quad_count * 6);
    for q in 0..quad_count as u32 {
        let base = q * 4;
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    indices
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placement(row: usize, col: usize, cols: usize, rows: usize, z: i32) -> GraphicsPlacement {
        GraphicsPlacement {
            row,
            col,
            cols,
            rows,
            image_id: 1,
            src_top_left: [0.0, 0.0],
            src_bottom_right: [1.0, 1.0],
            z_index: z,
        }
    }

    #[test]
    fn dst_rect_maps_cells_to_logical_pixels() {
        let p = placement(2, 3, 4, 5, 0);
        let rect = placement_dst_rect(&p, 10.0, 20.0, 8.0, 16.0);
        assert_eq!(
            rect,
            [10.0 + 3.0 * 8.0, 20.0 + 2.0 * 16.0, 4.0 * 8.0, 5.0 * 16.0]
        );
    }

    #[test]
    fn quad_vertices_wind_with_source_texcoords() {
        let p = placement(0, 0, 1, 1, 0);
        let quad = QueuedQuad {
            image_id: 1,
            z_index: 0,
            dst: placement_dst_rect(&p, 0.0, 0.0, 8.0, 16.0),
            src_top_left: [0.0, 0.0],
            src_bottom_right: [1.0, 1.0],
        };
        let verts = quad_vertices(&quad);
        assert_eq!(verts[0].position, [0.0, 0.0]);
        assert_eq!(verts[0].uv, [0.0, 0.0]);
        assert_eq!(verts[2].position, [8.0, 16.0]);
        assert_eq!(verts[2].uv, [1.0, 1.0]);
    }

    #[test]
    fn quad_indices_two_triangles_per_quad() {
        assert_eq!(quad_indices(1), vec![0, 1, 2, 0, 2, 3]);
        assert_eq!(quad_indices(2), vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7]);
    }
}
