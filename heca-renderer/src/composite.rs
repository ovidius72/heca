//! Compositor: a persistent offscreen **scene texture** the UI renders into, then
//! blits onto the swapchain.
//!
//! This is the foundation for **damage-region redraw**. A swapchain texture is not
//! preserved frame-to-frame, so you can't incrementally touch it up. The persistent
//! scene texture *is* preserved: a frame can re-render only the damaged region
//! (scissored) into it — unchanged pixels survive — and then a cheap full-screen
//! blit puts it on screen. Idle frames are skipped entirely (nothing damaged), and
//! an animating widget (a spinner, a caret) re-renders only its own rect instead of
//! forcing a whole-scene redraw.

/// Owns the persistent scene texture and the blit pipeline that copies it to the
/// swapchain.
pub struct Compositor {
    scene_tex: wgpu::Texture,
    scene_view: wgpu::TextureView,
    /// Stencil buffer paired with `scene_tex` (sized to match). Holds the rounded
    /// content-clip mask; see [`STENCIL_FORMAT`].
    stencil_tex: wgpu::Texture,
    stencil_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    size: (u32, u32),
    format: wgpu::TextureFormat,
}

/// Stencil buffer format paired with the scene texture. Used by the rounded
/// content-clip mechanism: a stencil-write pass marks each pane's inner rounded
/// rect, and the content renderers (backdrop/primitive/text) test against it so
/// terminal content follows the pane's rounded border instead of poking past it
/// at high corner radii. `Depth24PlusStencil8` is a core-guaranteed format.
pub const STENCIL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;

/// Depth/stencil state for content render passes (backdrop/primitive/text) that
/// test against the rounded content-clip mask written by [`GridRenderer::render_stencil`].
/// Stencil compare `Equal` to the pass reference (1), all ops `Keep` (content never
/// modifies the mask), depth disabled. Pipelines using this require their render
/// pass to attach the stencil view with `depth_ops: None` + `stencil_ops: Some(Load/Store)`
/// and call `set_stencil_reference(1)`.
pub fn content_clip_stencil_state() -> wgpu::DepthStencilState {
    let face = wgpu::StencilFaceState {
        compare: wgpu::CompareFunction::Equal,
        fail_op: wgpu::StencilOperation::Keep,
        depth_fail_op: wgpu::StencilOperation::Keep,
        pass_op: wgpu::StencilOperation::Keep,
    };
    wgpu::DepthStencilState {
        format: STENCIL_FORMAT,
        depth_write_enabled: false,
        depth_compare: wgpu::CompareFunction::Always,
        stencil: wgpu::StencilState {
            front: face,
            back: face,
            read_mask: 0xFFFFFFFF,
            write_mask: 0,
        },
        bias: wgpu::DepthBiasState::default(),
    }
}

impl Compositor {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("composite_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("composite.wgsl").into()),
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("composite_sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("composite_bind_group_layout"),
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
            label: Some("composite_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("composite_pipeline"),
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

        let (scene_tex, scene_view, bind_group) =
            Self::make_target(device, &bind_group_layout, &sampler, format, width, height);
        let (stencil_tex, stencil_view) = Self::make_stencil(device, width, height);

        Self {
            scene_tex,
            scene_view,
            stencil_tex,
            stencil_view,
            sampler,
            bind_group_layout,
            bind_group,
            pipeline,
            size: (width.max(1), height.max(1)),
            format,
        }
    }

    /// (Re)create the scene texture + its view + the blit bind group at `width`×`height`.
    fn make_target(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView, wgpu::BindGroup) {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene_texture"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("composite_bind_group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        (tex, view, bind_group)
    }

    /// Resize the persistent scene texture to match the framebuffer.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if (width.max(1), height.max(1)) == self.size {
            return;
        }
        let (tex, view, bind_group) = Self::make_target(
            device,
            &self.bind_group_layout,
            &self.sampler,
            self.format,
            width,
            height,
        );
        self.scene_tex = tex;
        self.scene_view = view;
        self.bind_group = bind_group;
        let (stencil_tex, stencil_view) = Self::make_stencil(device, width, height);
        self.stencil_tex = stencil_tex;
        self.stencil_view = stencil_view;
        self.size = (width.max(1), height.max(1));
    }

    /// (Re)create the stencil buffer paired with the scene texture at
    /// `width`×`height`. Depth is unused (depth ops are `None` everywhere); only
    /// the stencil aspect holds the rounded content-clip mask.
    fn make_stencil(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene_stencil"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: STENCIL_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        (tex, view)
    }

    /// The view the UI renders the scene into (instead of the swapchain).
    pub fn scene_view(&self) -> &wgpu::TextureView {
        &self.scene_view
    }

    /// The stencil buffer paired with the scene texture. Content renderers attach
    /// this and test against the rounded content-clip mask written each frame.
    pub fn stencil_view(&self) -> &wgpu::TextureView {
        &self.stencil_view
    }

    /// Blit the persistent scene texture onto `target` (the swapchain view).
    pub fn blit(&self, target: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("composite_blit"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
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
        rpass.draw(0..3, 0..1);
    }
}
