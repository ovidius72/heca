//! **The GPU a window draws with** — surface, device, queue and swap-chain configuration, set up
//! from a window the one way every surface needs it (the app and the showcase). One setup for every
//! window that draws: two copies of a setup drift, and every wgpu upgrade would have to find both.

use std::sync::Arc;

use winit::window::Window;

/// What drawing into one window takes, once the GPU is set up for it.
pub struct WindowGpu {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// The swap-chain configuration, already applied to `surface`. A resize changes its size and
    /// calls `surface.configure` again.
    pub config: wgpu::SurfaceConfiguration,
}

impl WindowGpu {
    /// Set up the GPU for `window`: any backend, the default adapter, an sRGB swap chain sized to the
    /// window, vsync. `label` names the device in GPU debuggers. `transparent` asks for a
    /// compositing mode that lets the desktop show through the window.
    pub async fn new(window: Arc<Window>, label: &str, transparent: bool) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let size = window.inner_size();
        let surface = instance
            .create_surface(window)
            .expect("Failed to create surface");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .expect("Failed to find an appropriate adapter");
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some(label),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
            })
            .await
            .expect("Failed to create device");

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: alpha_mode(&caps.alpha_modes, transparent),
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);
        Self { surface, device, queue, config }
    }

    /// The colour format every renderer draws in: the swap chain's.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }
}

/// The compositing alpha mode for a window. A transparent window wants the desktop to show through,
/// so it prefers premultiplied — matching the renderers' premultiplied blending — then
/// postmultiplied; an opaque one prefers `Opaque`. Either way it falls back to the first mode the
/// surface offers.
fn alpha_mode(modes: &[wgpu::CompositeAlphaMode], transparent: bool) -> wgpu::CompositeAlphaMode {
    use wgpu::CompositeAlphaMode::*;
    let preferred: &[wgpu::CompositeAlphaMode] = if transparent {
        &[PreMultiplied, PostMultiplied]
    } else {
        &[Opaque]
    };
    preferred
        .iter()
        .find(|mode| modes.contains(mode))
        .or(modes.first())
        .copied()
        .unwrap_or(Auto)
}

/// A device with no window, for tests that render into an offscreen texture.
#[cfg(test)]
pub(crate) fn headless_device() -> (wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))
        .expect("a GPU adapter for rendering tests");
    pollster::block_on(adapter.request_device(&Default::default())).expect("a rendering device")
}

#[cfg(test)]
mod tests {
    use super::alpha_mode;
    use wgpu::CompositeAlphaMode::*;

    #[test]
    fn a_transparent_window_prefers_a_blending_mode_and_an_opaque_one_prefers_opaque() {
        let offered = [Opaque, PostMultiplied];
        assert_eq!(alpha_mode(&offered, true), PostMultiplied);
        assert_eq!(alpha_mode(&[Opaque, PreMultiplied, PostMultiplied], true), PreMultiplied);
        assert_eq!(alpha_mode(&offered, false), Opaque);
        // Nothing preferred is offered: the surface's first mode, and `Auto` when it offers none.
        assert_eq!(alpha_mode(&[Inherit], true), Inherit);
        assert_eq!(alpha_mode(&[PostMultiplied], false), PostMultiplied);
        assert_eq!(alpha_mode(&[], false), Auto);
    }
}
