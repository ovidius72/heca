//! In-app separable Gaussian blur — a reusable post-process primitive.
//!
//! Given **any** source texture view, [`Blur::process`] produces a blurred copy
//! via two passes (horizontal then vertical) through internal ping-pong targets,
//! and returns the blurred view. It is **content-agnostic and host/compositor-
//! owned** (see `pluggable-chrome-plugin-plan.md` Phase 7.5 — blur is a compositor
//! concern, not terminal-owned): the intended app use is to frost chrome over the
//! scene, and a terminal/pane host can reuse it on its own offscreen content.
//!
//! **Status:** this is the standalone primitive only — it is **NOT yet wired into
//! the app** (no `Blur` field on `AppState`, no call site in `render_frame`), so
//! `appearance.blur` currently has no runtime effect. App-side wiring is a tracked
//! follow-up (`PLAN.md`).
//!
//! **Units:** `radius` is in the **source texture's own pixel space** — i.e.
//! *physical* pixels for the compositor scene texture (created at framebuffer size).
//! `heca-config`'s `appearance.blur_radius()` is in **logical** px, so a caller must
//! convert before calling: `radius_physical = blur_radius() * scale_factor`.
//!
//! Contract for reuse:
//! ```ignore
//! let mut blur = Blur::new(&device, format, w, h);
//! // on resize: blur.resize(&device, w, h);
//! let blurred: &wgpu::TextureView =
//!     blur.process(&device, &queue, &mut encoder, &source_view, radius);
//! // sample `blurred` (linear) as a frosted backdrop, then draw translucent chrome over it.
//! ```
//! The source must match the blur's current size; `radius` is in source-texture
//! pixels (see Units above; 0 = passthrough). No app types, no GPU globals.

use wgpu::util::DeviceExt;

/// Per-pass uniform: the per-tap UV step along the blur axis + the radius.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct BlurParams {
    step: [f32; 2],
    radius: f32,
    _pad: f32,
}

/// Reusable separable-Gaussian blur over offscreen textures.
pub struct Blur {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    // Ping-pong targets: pass 1 (H) → `a`, pass 2 (V) → `b` (returned).
    view_a: wgpu::TextureView,
    view_b: wgpu::TextureView,
    params_h: wgpu::Buffer,
    params_v: wgpu::Buffer,
    // Persistent bind group for the vertical pass (samples the internal `a`).
    bg_v: wgpu::BindGroup,
    size: (u32, u32),
    format: wgpu::TextureFormat,
}

impl Blur {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, width: u32, height: u32) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blur_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("blur.wgsl").into()),
        });

        // Linear sampling so the 9-tap kernel reads smoothly between texels.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("blur_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blur_bind_group_layout"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blur_pipeline_layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blur_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
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

        let params_h = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("blur_params_h"),
            contents: bytemuck::cast_slice(&[BlurParams { step: [0.0, 0.0], radius: 0.0, _pad: 0.0 }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let params_v = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("blur_params_v"),
            contents: bytemuck::cast_slice(&[BlurParams { step: [0.0, 0.0], radius: 0.0, _pad: 0.0 }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let (view_a, view_b, bg_v) =
            Self::make_targets(device, &layout, &sampler, &params_v, format, width, height);

        Self {
            pipeline,
            layout,
            sampler,
            view_a,
            view_b,
            params_h,
            params_v,
            bg_v,
            size: (width.max(1), height.max(1)),
            format,
        }
    }

    /// (Re)create the two ping-pong targets + the persistent vertical-pass bind group.
    fn make_targets(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        params_v: &wgpu::Buffer,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> (wgpu::TextureView, wgpu::TextureView, wgpu::BindGroup) {
        let make = |label: &str| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: width.max(1),
                        height: height.max(1),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        };
        let view_a = make("blur_target_a");
        let view_b = make("blur_target_b");
        let bg_v = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blur_bg_v"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view_a) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: params_v.as_entire_binding() },
            ],
        });
        (view_a, view_b, bg_v)
    }

    /// Resize the internal targets to match the source. No-op if unchanged.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if (width.max(1), height.max(1)) == self.size {
            return;
        }
        let (view_a, view_b, bg_v) = Self::make_targets(
            device,
            &self.layout,
            &self.sampler,
            &self.params_v,
            self.format,
            width,
            height,
        );
        self.view_a = view_a;
        self.view_b = view_b;
        self.bg_v = bg_v;
        self.size = (width.max(1), height.max(1));
    }

    /// Blur `src_view` (must match the current size) by `radius` in **source-texture
    /// pixels** (physical px for the compositor scene texture — convert from logical
    /// `appearance.blur_radius()` via `* scale_factor`; see module Units note) and
    /// return the blurred view. `radius <= 0` still copies (passthrough). The
    /// returned view is owned by `self` and valid until the next `process`/`resize`.
    pub fn process(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        src_view: &wgpu::TextureView,
        radius: f32,
    ) -> &wgpu::TextureView {
        let (w, h) = self.size;
        // Spread the 4-tap-per-side kernel across `radius` source pixels.
        let span = (radius.max(0.0)) / 4.0;
        queue.write_buffer(
            &self.params_h,
            0,
            bytemuck::cast_slice(&[BlurParams {
                step: [span / w as f32, 0.0],
                radius,
                _pad: 0.0,
            }]),
        );
        queue.write_buffer(
            &self.params_v,
            0,
            bytemuck::cast_slice(&[BlurParams {
                step: [0.0, span / h as f32],
                radius,
                _pad: 0.0,
            }]),
        );

        // Horizontal pass: external src → internal `a`. Bind group is built per
        // call because the source view is caller-owned and may change each frame.
        let bg_h = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blur_bg_h"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(src_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: self.params_h.as_entire_binding() },
            ],
        });
        self.pass(encoder, &bg_h, &self.view_a, "blur_pass_h");
        // Vertical pass: internal `a` → internal `b` (returned).
        self.pass(encoder, &self.bg_v, &self.view_b, "blur_pass_v");
        &self.view_b
    }

    fn pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        bind_group: &wgpu::BindGroup,
        target: &wgpu::TextureView,
        label: &str,
    ) {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, bind_group, &[]);
        rpass.draw(0..3, 0..1);
    }
}
