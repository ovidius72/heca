//! In-app Kawase blur — a reusable post-process primitive.
//!
//! Given **any** source texture view, [`Blur::process`] produces a blurred copy
//! via N compounding passes through internal ping-pong targets, and returns the
//! blurred view. It is **content-agnostic and host/compositor-owned** (see
//! `pluggable-chrome-plugin-plan.md` Phase 7.5 — blur is a compositor concern, not
//! terminal-owned): the intended app use is to frost chrome over the scene, and a
//! terminal/pane host can reuse it on its own offscreen content.
//!
//! **Why Kawase, not a separable Gaussian:** a single 9-tap separable Gaussian
//! undersamples a large radius — its taps sit `span = radius/4` px apart, so narrow
//! high-frequency content (a 2px-wide cursor, or fine text strokes) aliases through
//! as visible vertical/horizontal lines, and coarse features (text rows) survive
//! because the radius is too small to smear them. The Kawase filter instead runs N
//! passes with an *increasing* offset: each pass is a weighted 9-tap (center ×4 +
//! 4 cardinal ×2 + 4 diagonal ×1, ÷16) at a single offset `d` texels, and `d` grows
//! from small to the target radius across passes. Early small-offset passes smooth
//! fine detail so later large-offset passes blur already-smoothed content (no
//! aliasing), and the compounding offsets yield a smooth strong blur (σ ≈ √Σdᵢ²/3).
//! This is the standard frosted-glass technique. `BLUR_PASSES` passes (even, so the
//! final pass lands on the returned `view_b`); effective σ ≈ 1.0·radius at N=8.
//!
//! **wgpu uniform-update hazard (why `params` is a `Vec<Buffer>`):** `queue::write_buffer`
//! is a *queued* operation, while the render passes are encoded into the caller's
//! `CommandEncoder` (submitted only after `process` returns). All `write_buffer`
//! calls therefore execute *before* any pass runs — writing the *same* buffer N
//! times would leave it holding only the last offset, so every pass would read the
//! final value (no compounding, plus banding from a single large offset). Using one
//! *distinct* buffer per pass (each written once) sidesteps the hazard entirely:
//! each pass reads its own buffer's value regardless of queue order.
//!
//! **Status:** wired into the app — `Blur` and `Backdrop` are owned by `AppState`
//! and called from `render_frame()` in `heca/src/app/render.rs`. When
//! `appearance.terminal_floating_blur > 0` (with translucent floating panes), a blur
//! is performed after tiled panes render and stamped behind the floating panes,
//! frosting the actual tiled content behind them.
//!
//! **Units:** `radius` is in the **source texture's own pixel space** — i.e.
//! *physical* pixels for the compositor scene texture (created at framebuffer size).
//! `heca-config`'s `appearance.terminal_floating_blur_radius()` is in **logical**
//! px, so a caller must convert before calling:
//! `radius_physical = terminal_floating_blur_radius() * scale_factor`.
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

/// Number of compounding Kawase passes per `process` call (when `radius > 0`).
/// Must be **even** so the final pass writes `view_b` (the returned view). Eight
/// passes gives σ ≈ 1.0·radius — strong enough to frost coarse text rows while
/// keeping per-pass offsets small enough to avoid aliasing (early passes sample
/// densely). Each pass is a single 9-tap draw (not separable H+V), so cost is
/// `BLUR_PASSES` fullscreen passes per `process` call — cheap on modern GPUs and
/// only run while floating frost is active.
const BLUR_PASSES: u32 = 8;

/// Per-pass uniform: the Kawase offset in UV units `(offset/width, offset/height)`
/// plus the radius (used only as a passthrough gate; the offset drives the spread).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct BlurParams {
    step: [f32; 2],
    radius: f32,
    _pad: f32,
}

/// Reusable Kawase blur over offscreen ping-pong textures.
pub struct Blur {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    // Ping-pong targets. Pass 1 writes `view_a` (from the external src); even
    // passes write `view_b`, odd passes (>1) write `view_a`. With `BLUR_PASSES`
    // even, the final pass writes `view_b` (returned).
    view_a: wgpu::TextureView,
    view_b: wgpu::TextureView,
    // One params uniform *per pass* (see the wgpu hazard doc above): each is
    // written once with that pass's offset, so every pass reads its own value
    // regardless of the queue's write-before-submit ordering. Bind groups are
    // built per pass (cheap, descriptor-only) binding the matching buffer.
    params: Vec<wgpu::Buffer>,
    size: (u32, u32),
    format: wgpu::TextureFormat,
}

impl Blur {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
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

        // One params buffer per pass (see the `params` field doc + module hazard note).
        let params: Vec<wgpu::Buffer> = (0..BLUR_PASSES)
            .map(|_| {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("blur_params"),
                    contents: bytemuck::cast_slice(&[BlurParams {
                        step: [0.0, 0.0],
                        radius: 0.0,
                        _pad: 0.0,
                    }]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                })
            })
            .collect();

        let (view_a, view_b) = Self::make_targets(device, format, width, height);

        Self {
            pipeline,
            layout,
            sampler,
            view_a,
            view_b,
            params,
            size: (width.max(1), height.max(1)),
            format,
        }
    }

    /// (Re)create the two ping-pong targets.
    fn make_targets(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> (wgpu::TextureView, wgpu::TextureView) {
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
        (make("blur_target_a"), make("blur_target_b"))
    }

    /// Resize the internal targets to match the source. No-op if unchanged.
    /// (The per-pass params buffers are fixed-size uniforms — no resize needed.)
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if (width.max(1), height.max(1)) == self.size {
            return;
        }
        let (view_a, view_b) = Self::make_targets(device, self.format, width, height);
        self.view_a = view_a;
        self.view_b = view_b;
        self.size = (width.max(1), height.max(1));
    }

    /// Blur `src_view` (must match the current size) by `radius` in **source-texture
    /// pixels** (physical px for the compositor scene texture — convert from logical
    /// `appearance.terminal_floating_blur_radius()` via `* scale_factor`; see module
    /// Units note) and return the blurred view. `radius <= 0` still copies
    /// (passthrough via two zero-offset passes). The returned view is owned by
    /// `self` and valid until the next `process`/`resize`.
    pub fn process(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        src_view: &wgpu::TextureView,
        radius: f32,
    ) -> &wgpu::TextureView {
        let (w, h) = self.size;
        // Two zero-offset passes when radius <= 0 so `view_b` ends up holding a
        // plain copy of the source (the shader returns the raw sample at radius 0).
        let passes: u32 = if radius <= 0.0 { 2 } else { BLUR_PASSES };
        debug_assert!(
            (self.params.len() as u32) >= passes,
            "need {} params buffers, have {}",
            passes,
            self.params.len()
        );

        // Write each pass's offset to its own buffer (distinct buffers → no
        // last-write-wins hazard; each is read by exactly one pass).
        let zero = [0.0f32; 2];
        for i in 0..passes as usize {
            let offset = Self::offset_for(radius, (i + 1) as u32, passes);
            queue.write_buffer(
                &self.params[i],
                0,
                bytemuck::cast_slice(&[BlurParams {
                    step: if radius <= 0.0 {
                        zero
                    } else {
                        [offset / w.max(1) as f32, offset / h.max(1) as f32]
                    },
                    radius,
                    _pad: 0.0,
                }]),
            );
        }

        // Pass 1: external src → `view_a`.
        let bg = self.make_bg(device, src_view, &self.params[0]);
        self.pass(encoder, &bg, &self.view_a, "blur_pass_1");

        // Passes 2..N: ping-pong view_a ↔ view_b. Even passes → view_b; odd passes
        // (>1) → view_a. The encoder tracks render-pass resource usage and inserts
        // the read-after-write barriers between passes automatically.
        for p in 2..=passes {
            let (src, target) = if p % 2 == 0 {
                (&self.view_a, &self.view_b)
            } else {
                (&self.view_b, &self.view_a)
            };
            let bg = self.make_bg(device, src, &self.params[(p - 1) as usize]);
            self.pass(encoder, &bg, target, &format!("blur_pass_{}", p));
        }

        // `BLUR_PASSES` is even, so the final pass wrote `view_b`.
        &self.view_b
    }

    /// Kawase offset (in source-texture pixels) for pass `i` of `passes`: grows
    /// linearly from `radius/passes` (pass 1) to `radius` (pass N), so early passes
    /// sample densely (smooth fine detail) and later passes spread widely.
    /// Zero when `radius <= 0` (passthrough).
    fn offset_for(radius: f32, i: u32, passes: u32) -> f32 {
        if radius <= 0.0 || passes == 0 {
            0.0
        } else {
            radius * i as f32 / passes as f32
        }
    }

    /// Build a per-pass bind group binding `src` + this pass's `params` buffer.
    fn make_bg(
        &self,
        device: &wgpu::Device,
        src: &wgpu::TextureView,
        params: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blur_bg"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(src),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params.as_entire_binding(),
                },
            ],
        })
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
