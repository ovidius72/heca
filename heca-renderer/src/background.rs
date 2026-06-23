//! z=0 background layer — heca-owned frosted-glass background.
//!
//! [`BackgroundLayer`] owns the z=0 layer: it renders a 2-color vertical gradient
//! into an offscreen target, blurs it once (via the **shared** [`Blur`](crate::blur::Blur)),
//! and caches the blurred result. Panes then composite translucently over the
//! cached view, so the frost is heca-owned and identical cross-platform (no
//! OS-vibrancy dependency — see `compositor-blur-refactor-plan.md`).
//!
//! **Static + cached:** the gradient is static, so the blur is recomputed only
//! when something changes — `resize` (new framebuffer size) or `set_params` (new
//! gradient colors / blur radius). Between dirtying events, [`BackgroundLayer::render`]
//! returns the cached view with **no GPU work**. The cache is snapshotted from the
//! shared blur's returned `view_b` via a fullscreen blit pass, so the cache stays
//! valid even after the shared blur is reused later in the same frame (the
//! floating-pane frost path calls `blur.process` again, overwriting `view_b`).
//!
//! **Critical (caller contract):** the shared `Blur` is reused for the z=0 blur
//! *and* the floating-pane blur. Because this layer snapshots the blurred result
//! into its own cache inside `render`, the caller is free to call the floating
//! `blur.process` afterwards — the cached z=0 view is independent of the shared
//! blur's internal ping-pong textures.
//!
//! Headless primitive (no `wgpu::Surface`, no app types). The caller (the app)
//! owns the `Blur` and passes it in, so no dedicated ping-pong pair is allocated
//! here — only the z=0 gradient target + the cache target.

use crate::blur::Blur;
use crate::gradient::GradientRenderer;

/// z=0 layer state: gradient target, blurred-result cache, dirty flag, cached params.
pub struct BackgroundLayer {
    gradient: GradientRenderer,
    z0_tex: wgpu::Texture,
    z0_view: wgpu::TextureView,
    cache_tex: wgpu::Texture,
    cache_view: wgpu::TextureView,
    // Blit pipeline: copies the shared blur's returned view into `cache_tex`.
    blit_pipeline: wgpu::RenderPipeline,
    blit_layout: wgpu::BindGroupLayout,
    blit_sampler: wgpu::Sampler,
    dirty: bool,
    top: [f32; 4],
    bottom: [f32; 4],
    blur_radius: f32,
    size: (u32, u32),
    format: wgpu::TextureFormat,
}

impl BackgroundLayer {
    /// Construct the z=0 layer at `(width, height)` physical px for the given
    /// target `format`. Starts `dirty` so the first `render` computes the gradient
    /// + blur. `width`/`height` are clamped to ≥ 1.
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let gradient = GradientRenderer::new(device, format);

        let (z0_tex, z0_view) = Self::make_target(device, format, width, height, "z0");
        let (cache_tex, cache_view) = Self::make_target(device, format, width, height, "cache");

        let blit_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("background_blit_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("background_blit_layout"),
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
            label: Some("background_blit_pipeline_layout"),
            bind_group_layouts: &[&blit_layout],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("background_blit_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("background.wgsl").into()),
        });

        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("background_blit_pipeline"),
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

        Self {
            gradient,
            z0_tex,
            z0_view,
            cache_tex,
            cache_view,
            blit_pipeline,
            blit_layout,
            blit_sampler,
            dirty: true,
            top: [0.0, 0.0, 0.0, 1.0],
            bottom: [0.0, 0.0, 0.0, 1.0],
            blur_radius: 0.0,
            size: (width.max(1), height.max(1)),
            format,
        }
    }

    /// Make a z=0 / cache target: needs `RENDER_ATTACHMENT` (gradient + blit write)
    /// and `TEXTURE_BINDING` (blur reads z=0; the app samples cache_view when
    /// compositing z=0 into the scene).
    fn make_target(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        label: &str,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        (tex, view)
    }

    /// Resize the z=0 + cache targets to match the framebuffer. Sets `dirty` so the
    /// gradient + blur recompute next frame. No-op if the size is unchanged.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let next = (width.max(1), height.max(1));
        if next == self.size {
            return;
        }
        let (z0_tex, z0_view) = Self::make_target(device, self.format, width, height, "z0");
        let (cache_tex, cache_view) =
            Self::make_target(device, self.format, width, height, "cache");
        self.z0_tex = z0_tex;
        self.z0_view = z0_view;
        self.cache_tex = cache_tex;
        self.cache_view = cache_view;
        self.size = next;
        self.dirty = true;
    }

    /// Pure helper: whether the given params differ from the cached ones. Factored
    /// out (delegates to the free [`params_differ`]) so the dirty-flag decision is
    /// unit-testable without a GPU device.
    pub fn params_changed(&self, top: [f32; 4], bottom: [f32; 4], blur_radius: f32) -> bool {
        params_differ(
            (self.top, self.bottom, self.blur_radius),
            (top, bottom, blur_radius),
        )
    }

    /// Update the gradient colors + blur radius. Sets `dirty` if anything changed
    /// (so the next `render` recomputes the gradient + blur).
    pub fn set_params(&mut self, top: [f32; 4], bottom: [f32; 4], blur_radius: f32) {
        if self.params_changed(top, bottom, blur_radius) {
            self.top = top;
            self.bottom = bottom;
            self.blur_radius = blur_radius;
            self.dirty = true;
        }
    }

    /// Render the z=0 layer and return the cached blurred view (the caller composites
    /// it into the scene at alpha = `background_alpha()`). If not dirty, returns the
    /// cached view with **no GPU work**. If dirty: gradient → `z0`; `blur.process(z0)`
    /// → blurred `view_b`; blit blurred → `cache`; clear `dirty`. The returned view is
    /// owned by `self` and valid until the next `resize`.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        blur: &Blur,
    ) -> &wgpu::TextureView {
        if !self.dirty {
            return &self.cache_view;
        }

        // 1. Gradient → z0.
        self.gradient
            .render(queue, encoder, &self.z0_view, self.top, self.bottom);

        // 2. z0 → blurred (shared blur). `radius <= 0` is a passthrough (two
        //    zero-offset passes) so an un-blurred gradient still lands in the cache.
        let blurred = blur.process(device, queue, encoder, &self.z0_view, self.blur_radius);

        // 3. Blit blurred → cache (snapshot, so the cache survives the floating-pane
        //    blur reusing `blur` later this frame).
        let blit_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("background_blit_bg"),
            layout: &self.blit_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(blurred),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.blit_sampler),
                },
            ],
        });
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("background_blit_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.cache_view,
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
        rpass.set_pipeline(&self.blit_pipeline);
        rpass.set_bind_group(0, &blit_bg, &[]);
        rpass.draw(0..3, 0..1);
        drop(rpass);

        self.dirty = false;
        &self.cache_view
    }

    /// Whether the layer is dirty (next `render` will recompute). Exposed for tests.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Current layer size in physical px.
    pub fn size(&self) -> (u32, u32) {
        self.size
    }
}

/// Free comparison of two `(top, bottom, blur_radius)` param triples. Used by
/// [`BackgroundLayer::params_changed`] so the dirty-flag logic is unit-testable
/// without constructing a layer (which needs a GPU device).
fn params_differ(a: ([f32; 4], [f32; 4], f32), b: ([f32; 4], [f32; 4], f32)) -> bool {
    a.0 != b.0 || a.1 != b.1 || a.2 != b.2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_differ_detects_color_and_blur_change() {
        let top = [0.1, 0.2, 0.3, 1.0];
        let bottom = [0.9, 0.8, 0.7, 1.0];
        let blur = 12.0_f32;

        // Identical params → no change (steady state, no dirty).
        assert!(!params_differ((top, bottom, blur), (top, bottom, blur)));
        // Top color differs → change.
        assert!(params_differ(
            (top, bottom, blur),
            ([0.1, 0.2, 0.31, 1.0], bottom, blur),
        ));
        // Bottom color differs → change.
        assert!(params_differ(
            (top, bottom, blur),
            (top, [0.9, 0.8, 0.71, 1.0], blur),
        ));
        // Blur radius differs → change.
        assert!(params_differ((top, bottom, blur), (top, bottom, 13.0)));
        // Alpha differs → change (translucency edits must reblur).
        assert!(params_differ(
            (top, bottom, blur),
            ([0.1, 0.2, 0.3, 0.5], bottom, blur),
        ));
    }

    /// Pins the default-state contract: a fresh layer caches opaque-black gradient
    /// colors with blur 0, so reporting the same values means "no change".
    #[test]
    fn default_params_match_opaque_black_zero_blur() {
        let dflt = ([0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 1.0], 0.0);
        assert!(!params_differ(dflt, dflt));
    }
}
