//! Backdrop sampler — draw a region of a source texture into a destination rect.
//!
//! The reusable **"draw blurred backdrop into a rect"** stage that pairs with
//! [`Blur`](crate::blur::Blur). `Blur` produces a blurred copy of the scene;
//! `Backdrop` stamps a slice of that (or any texture) into an arbitrary on-screen
//! rect, alpha-blended, so a pane/chrome region can use it as a frosted backdrop
//! and then draw its own translucent content on top.
//!
//! It is **content-agnostic and host/compositor-owned** (terminal panes reuse it;
//! they don't roll their own — see `pluggable-chrome-plugin-plan.md` Phase 7.5).
//!
//! Typical frosted-pane flow (per frame):
//! ```ignore
//! let blurred = blur.process(&device, &queue, &mut enc, scene_view, radius_px);
//! // stamp the blurred content *behind* this pane into the pane's rect:
//! backdrop.draw(&device, &queue, &mut enc, target, blurred,
//!               viewport_px, pane_rect_px, None /* = same screen location */, 1.0);
//! // …then draw the pane's translucent surface + content over `target`.
//! ```
//!
//! Coordinates are **physical pixels** (matching the compositor textures); pass the
//! framebuffer size as `viewport_px`. `src_uv` is optional: `None` samples the same
//! screen location as `dst` (the common "frost what's directly behind me" case).

use wgpu::util::DeviceExt;

use crate::composite::content_clip_stencil_state;

/// A rectangle in physical pixels: `(x, y, w, h)`, origin top-left.
pub type RectPx = (f32, f32, f32, f32);

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    dst_min: [f32; 2],
    dst_max: [f32; 2],
    uv_min: [f32; 2],
    uv_max: [f32; 2],
    opacity_pad: [f32; 4],
}

/// Draws a (sub-region of a) texture into a destination rect, alpha-blended.
pub struct Backdrop {
    pipeline: wgpu::RenderPipeline,
    /// Stencil-test variant: same as `pipeline` but tests `Equal` against the rounded
    /// content-clip mask. Used when `draw` is given a stencil view.
    stencil_pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl Backdrop {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("backdrop_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("backdrop.wgsl").into()),
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("backdrop_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("backdrop_bind_group_layout"),
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
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
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
            label: Some("backdrop_pipeline_layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("backdrop_pipeline"),
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
                    // Standard alpha blend so a faded/edge-feathered backdrop and
                    // the translucent content drawn afterwards composite correctly.
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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

        // Stencil-test variant: identical to `pipeline` but tests the rounded
        // content-clip mask (`Equal` to ref 1, set per pass). Depth disabled.
        let stencil_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("backdrop_stencil_pipeline"),
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
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(content_clip_stencil_state()),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            stencil_pipeline,
            layout,
            sampler,
        }
    }

    /// Stamp a region of `src` into `dst` (physical px) on `target`, at `opacity`.
    ///
    /// `src_uv` selects the source region in UV space (0..1, origin top-left);
    /// `None` samples the **same screen location** as `dst` (frost what's directly
    /// behind the rect). `viewport_px` is the target/framebuffer size in physical px.
    #[expect(
        clippy::too_many_arguments,
        reason = "backdrop draws need explicit texture, viewport, rect, opacity, and stencil inputs"
    )]
    pub fn draw(
        &self,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        src: &wgpu::TextureView,
        viewport_px: (f32, f32),
        dst: RectPx,
        src_uv: Option<[f32; 4]>,
        opacity: f32,
        stencil: Option<&wgpu::TextureView>,
    ) {
        let (vw, vh) = viewport_px;
        if vw <= 0.0 || vh <= 0.0 || dst.2 <= 0.0 || dst.3 <= 0.0 {
            return;
        }
        let (x, y, w, h) = dst;
        // Destination px → NDC (y flipped: screen-top = +1).
        let ndc = |px: f32, py: f32| -> (f32, f32) { (px / vw * 2.0 - 1.0, 1.0 - py / vh * 2.0) };
        let (l, t) = ndc(x, y);
        let (r, b) = ndc(x + w, y + h);
        // Source UV: explicit, or the same screen location as dst (origin top-left).
        let uv = src_uv.unwrap_or([x / vw, y / vh, (x + w) / vw, (y + h) / vh]);

        let params = Params {
            dst_min: [l, t],
            dst_max: [r, b],
            uv_min: [uv[0], uv[1]],
            uv_max: [uv[2], uv[3]],
            opacity_pad: [opacity.clamp(0.0, 1.0), 0.0, 0.0, 0.0],
        };
        // Per-draw uniform + bind group so multiple backdrops can be drawn in one
        // frame/encoder without clobbering each other (src view is caller-owned).
        let ubo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("backdrop_params"),
            contents: bytemuck::cast_slice(&[params]),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("backdrop_bind_group"),
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
                    resource: ubo.as_entire_binding(),
                },
            ],
        });

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("backdrop_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
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
        rpass.set_bind_group(0, &bind_group, &[]);
        if stencil.is_some() {
            rpass.set_stencil_reference(1);
        }
        rpass.draw(0..6, 0..1);
    }
}
