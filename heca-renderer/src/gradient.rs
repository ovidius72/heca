//! Gradient fill — a reusable GPU primitive that fills a render target with a
//! 2-color **vertical** (top→bottom) gradient via a fullscreen-triangle pipeline.
//!
//! This is the z=0 background layer's *source*: [`crate::background::BackgroundLayer`]
//! renders the gradient into an offscreen target, then blurs it. It is a headless
//! primitive (no `wgpu::Surface`, no app types) — colors are linear `f32x4`,
//! matching the `primitive_renderer` convention.
//!
//! Contract:
//! ```ignore
//! let gradient = GradientRenderer::new(&device, format);
//! gradient.render(&queue, &mut encoder, &target_view, top_rgba, bottom_rgba);
//! ```
//! The caller owns the render-target texture/view (created with
//! `RENDER_ATTACHMENT`). Colors are written as-is (no premultiply); pass alpha < 1
//! only if the gradient itself should be translucent.

use wgpu::util::DeviceExt;

/// Per-draw uniform: the two gradient colors (linear `f32x4`).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GradientParams {
    top: [f32; 4],
    bottom: [f32; 4],
}

/// Fills a render-target texture view with a 2-color vertical gradient
/// (top→bottom). One fullscreen-triangle draw; no vertex buffer. The pipeline is
/// format-agnostic (built for one `TextureFormat`); colors are a uniform updated
/// per `render` call.
pub struct GradientRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    params: wgpu::Buffer,
}

impl GradientRenderer {
    /// Construct the gradient pipeline for the given target `format`.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gradient_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gradient.wgsl").into()),
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gradient_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gradient_pipeline_layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gradient_pipeline"),
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

        let params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("gradient_params"),
            contents: bytemuck::cast_slice(&[GradientParams {
                top: [0.0, 0.0, 0.0, 1.0],
                bottom: [0.0, 0.0, 0.0, 1.0],
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gradient_bind_group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: params.as_entire_binding(),
            }],
        });

        Self {
            pipeline,
            bind_group,
            params,
        }
    }

    /// Fill `target` with a vertical gradient from `top` (screen top) to `bottom`
    /// (screen bottom). Colors are linear `f32x4`. The render pass clears the
    /// target to transparent first, then draws the fullscreen triangle over it
    /// (so the whole target is the gradient — no stale content).
    pub fn render(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        top: [f32; 4],
        bottom: [f32; 4],
    ) {
        queue.write_buffer(
            &self.params,
            0,
            bytemuck::cast_slice(&[GradientParams { top, bottom }]),
        );

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("gradient_pass"),
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
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{align_of, offset_of, size_of};

    /// Pins the `#[repr(C)]` layout written to the GPU uniform buffer so a field
    /// reorder/add that silently mismatches the WGSL `Params` struct is caught
    /// here, not as a wrong-color render at runtime.
    #[test]
    fn gradient_params_layout_matches_wgsl_uniform() {
        assert_eq!(size_of::<GradientParams>(), 32);
        assert_eq!(align_of::<GradientParams>(), 4);
        assert_eq!(offset_of!(GradientParams, top), 0);
        assert_eq!(offset_of!(GradientParams, bottom), 16);
    }
}