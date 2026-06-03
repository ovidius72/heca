//! Grid UI showcase — the first runnable visual proof of Phase B.
//!
//! Builds a small `heca-grid-ui` component tree (a row of glowing HUD cards),
//! lays it out with the layout engine, paints it into a `Scene`, and renders the
//! `Scene` via the GPU backend (SDF glow + corner brackets + scanline) on a dark
//! Grid theme.
//!
//! Run: `cargo run -p heca-renderer --example showcase`

use std::sync::Arc;

use heca_grid_ui::scene::{BracketCmd, DrawCommand, Glow, ScanlineCmd};
use heca_grid_ui::{
    Align, Component, Flex, Label, LayoutEngine, Length, PaintCx, Rectangle, Scene, Size, Theme,
};
use heca_renderer::grid::GridRenderer;
use heca_renderer::scene::enqueue_scene;
use heca_renderer::text::TextRenderer;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

/// Build the showcase UI for the current logical window size.
fn build_ui(w: f32, h: f32, theme: &Theme) -> Flex {
    let card = |title: &str, value: &str| {
        Flex::column()
            .width(Length::Px(240.0))
            .height(Length::Px(150.0))
            .padding(18.0)
            .gap(10.0)
            .background(theme.surface)
            .border(theme.accent, 1.5)
            .glow(theme.glow)
            .radius(4.0)
            .child(Label::new(title).color(theme.muted).font_size(13.0))
            .child(Label::new(value).color(theme.foreground).font_size(34.0))
    };

    Flex::row()
        .width(Length::Px(w))
        .height(Length::Px(h))
        .padding(48.0)
        .gap(28.0)
        .align(Align::Center)
        .child(card("UPLINK", "ONLINE"))
        .child(card("POWER", "98%"))
        .child(card("GRID NODES", "1024"))
}

/// Paint the UI tree into a scene, then decorate with brackets + scanlines.
fn build_scene(root: &Flex, theme: &Theme, w: f32, h: f32) -> Scene {
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, theme);
        root.paint(&mut cx);
    }
    // Corner brackets framing each card.
    for card in &root.base().children {
        scene.push(DrawCommand::Brackets(BracketCmd {
            rect: card.base().bounds,
            color: theme.accent,
            len: 14.0,
            thickness: 1.5,
            glow: Some(Glow {
                color: theme.glow,
                radius: 6.0,
                intensity: 1.0,
            }),
        }));
    }
    // Full-window scanline overlay.
    scene.push(DrawCommand::Scanline(ScanlineCmd {
        rect: Rectangle::from_size(Size::new(w as f64, h as f64)),
        color: theme.accent,
        spacing: 3.0,
        opacity: theme.intensity.scanline_opacity().max(0.05),
    }));
    scene
}

struct GpuState {
    window: Arc<Window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    grid: GridRenderer,
    text: TextRenderer,
    scale_factor: f64,
    theme: Theme,
}

impl GpuState {
    async fn new(event_loop: &ActiveEventLoop) -> Self {
        let attrs = Window::default_attributes()
            .with_title("heca-grid-ui showcase")
            .with_inner_size(winit::dpi::LogicalSize::new(900.0, 360.0));
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let scale_factor = window.scale_factor();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let surface = instance.create_surface(window.clone()).expect("surface");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("adapter");
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("showcase_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .expect("device");

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);
        let phys = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: phys.width.max(1),
            height: phys.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let theme = Theme::grid_tron();
        let grid = GridRenderer::new(&device, format);
        let mut text = TextRenderer::new(&device, format);
        text.set_scale_factor(scale_factor);
        text.set_font_family(&theme.font_family);

        Self {
            window,
            device,
            queue,
            surface,
            config,
            grid,
            text,
            scale_factor,
            theme,
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
        self.window.request_redraw();
    }

    fn render(&mut self) {
        let phys = self.window.inner_size();
        let scale = self.scale_factor as f32;
        let (w, h) = (phys.width as f32 / scale, phys.height as f32 / scale);

        self.grid.set_screen_size(&self.queue, w, h);
        self.text.set_screen_size(&self.queue, w, h);

        let mut root = build_ui(w, h, &self.theme);
        LayoutEngine::new().compute(&mut root, Size::new(w as f64, h as f64));
        let scene = build_scene(&root, &self.theme, w, h);
        enqueue_scene(&mut self.grid, &mut self.text, &scene);

        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(_) => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("showcase") });

        // Clear to the theme background.
        let bg = self.theme.background.to_f32x4();
        {
            let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: bg[0] as f64,
                            g: bg[1] as f64,
                            b: bg[2] as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
        }

        self.grid.render(&self.device, &view, &mut encoder);
        self.text.render(&self.device, &self.queue, &view, &mut encoder);
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }
}

#[derive(Default)]
struct App {
    state: Option<GpuState>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            let state = pollster::block_on(GpuState::new(event_loop));
            state.window.request_redraw();
            self.state = Some(state);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.resize(size.width, size.height),
            WindowEvent::RedrawRequested => state.render(),
            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("run");
}
