use heca_config::theme::AppConfig;
use heca_core::layout::MockLayout;
use heca_renderer::primitive::PrimitiveRenderer;
use heca_renderer::text::TextRenderer;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

struct AppState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    primitive_renderer: PrimitiveRenderer,
    text_renderer: TextRenderer,
    mock_layout: MockLayout,
    theme: heca_config::theme::Theme,
    scale_factor: f64,
    needs_redraw: bool,
}

struct HecaApp {
    state: Option<AppState>,
    app_config: AppConfig,
}

impl HecaApp {
    fn new() -> Self {
        let app_config = AppConfig::load();
        Self {
            state: None,
            app_config,
        }
    }

    async fn init_state(&mut self, event_loop: &ActiveEventLoop) -> AppState {
        let window_attrs = Window::default_attributes()
            .with_title("heca")
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.app_config.config.general.window_width as f64,
                self.app_config.config.general.window_height as f64,
            ));
        let window = Arc::new(event_loop.create_window(window_attrs).unwrap());
        let scale_factor = window.scale_factor();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find an appropriate adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("heca_device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                    trace: wgpu::Trace::Off,
                },
            )
            .await
            .expect("Failed to create device");

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let mut primitive_renderer = PrimitiveRenderer::new(&device, surface_format);
        let mut text_renderer = TextRenderer::new(&device, surface_format);
        text_renderer.set_scale_factor(scale_factor);
        text_renderer.set_screen_size(&queue, size.width as f32, size.height as f32);
        primitive_renderer.set_screen_size(&queue, size.width as f32, size.height as f32);

        let mock_layout = MockLayout::default_demo();

        AppState {
            window,
            surface,
            device,
            queue,
            config,
            primitive_renderer,
            text_renderer,
            mock_layout,
            theme: self.app_config.theme.clone(),
            scale_factor,
            needs_redraw: true,
        }
    }

    fn render(&mut self) {
        let state = self.state.as_mut().unwrap();
        if !state.needs_redraw {
            return;
        }
        state.needs_redraw = false;

        let surface_texture = match state.surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Lost) => {
                state.surface.configure(&state.device, &state.config);
                return;
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                std::process::exit(1);
            }
            Err(e) => {
                eprintln!("Surface error: {:?}", e);
                return;
            }
        };

        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let theme_bg = state.theme.background.to_f32x4();
        let theme_fg = state.theme.foreground.to_f32x4();
        let theme_border = state.theme.border.to_f32x4();
        let border_width = state.theme.border_width;

        let size = state.window.inner_size();
        let (pane_rects, float_rect) = state
            .mock_layout
            .compute_rects(size.width as f32, size.height as f32);

        let mut encoder = state
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render_encoder"),
            });

        // Clear background
        {
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: theme_bg[0] as f64,
                            g: theme_bg[1] as f64,
                            b: theme_bg[2] as f64,
                            a: theme_bg[3] as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
        }

        // Draw panes
        for (rect, pane) in &pane_rects {
            state.primitive_renderer.draw_rect(
                rect.x, rect.y, rect.w, rect.h,
                pane.background,
            );
            state.primitive_renderer.draw_border(
                rect.x, rect.y, rect.w, rect.h,
                theme_border, border_width,
            );
            state.text_renderer.queue_text(
                &pane.title,
                rect.x + 8.0,
                rect.y + 4.0,
                state.theme.font_size,
                theme_fg,
            );
        }

        // Draw float pane
        if let Some((rect, pane)) = float_rect {
            state.primitive_renderer.draw_rect(
                rect.x, rect.y, rect.w, rect.h,
                pane.background,
            );
            state.primitive_renderer.draw_border(
                rect.x, rect.y, rect.w, rect.h,
                theme_border, border_width * 2.0,
            );
            state.text_renderer.queue_text(
                &pane.title,
                rect.x + 8.0,
                rect.y + 4.0,
                state.theme.font_size,
                theme_fg,
            );
        }

        state.primitive_renderer.render(&state.device, &view, &mut encoder);
        state.text_renderer.render(
            &state.device, &state.queue, &view, &mut encoder,
        );

        state.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();
    }
}

impl ApplicationHandler for HecaApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            let state = pollster::block_on(self.init_state(event_loop));
            self.state = Some(state);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let state = match self.state.as_mut() {
            Some(s) => s,
            None => return,
        };

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                if new_size.width > 0 && new_size.height > 0 {
                    state.config.width = new_size.width;
                    state.config.height = new_size.height;
                    state.surface.configure(&state.device, &state.config);
                    state.primitive_renderer.set_screen_size(
                        &state.queue, new_size.width as f32, new_size.height as f32,
                    );
                    state.text_renderer.set_screen_size(
                        &state.queue, new_size.width as f32, new_size.height as f32,
                    );
                    state.needs_redraw = true;
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                state.scale_factor = scale_factor;
                state.text_renderer.set_scale_factor(scale_factor);
                state.needs_redraw = true;
            }
            WindowEvent::RedrawRequested => {
                state.needs_redraw = true;
                self.render();
            }
            WindowEvent::CursorMoved { .. }
            | WindowEvent::MouseInput { .. }
            | WindowEvent::KeyboardInput { .. } => {
                state.needs_redraw = true;
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = self.state.as_ref() {
            if state.needs_redraw {
                state.window.request_redraw();
            }
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = HecaApp::new();
    event_loop.run_app(&mut app).unwrap();
}
