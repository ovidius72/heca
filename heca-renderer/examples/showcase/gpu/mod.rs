use super::*;
mod input;
mod render;

impl GpuState {
    pub(crate) async fn new(event_loop: &ActiveEventLoop) -> Self {
        let attrs = Window::default_attributes()
            .with_title(APP_TITLE)
            .with_inner_size(winit::dpi::LogicalSize::new(900.0, 420.0));
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        // Bridge widget self-invalidation to the event loop: a widget that marks
        // itself needs-paint wakes the renderer through this (the retained-render
        // foundation — repaint what changed instead of every frame).
        {
            let w = window.clone();
            heca_grid_ui::install_frame_request(move || w.request_redraw());
        }
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

        let theme = load_grid_theme("grid_tron");
        let compositor =
            heca_renderer::composite::Compositor::new(&device, format, config.width, config.height);
        let mut grid = GridRenderer::new(&device, format);
        let mut text = TextRenderer::new(&device, format);
        // Both renderers need the scale + physical framebuffer size to map logical
        // clip rects to scissor pixels (clamped in-bounds).
        grid.set_scale_factor(scale_factor);
        grid.set_target_size(config.width, config.height);
        text.set_scale_factor(scale_factor);
        text.set_target_size(config.width, config.height);
        text.set_font_family(&theme.font_family);
        let ctl = ThemeCtl {
            glow: signal(theme.colors.glow_size),
            radius: signal(theme.colors.border_radius),
            border: signal(theme.colors.border_width),
            font: signal(theme.font_size),
            intensity: signal(theme.colors.intensity),
            overlay_frame: signal(theme.colors.overlay_frame),
            size: signal(WidgetSize::Normal),
            zoom: signal(ZOOM_DEFAULT),
            theme_idx: signal(0),
        };
        let built = build_ui(&theme, ctl);
        let BuiltUi {
            ui,
            overlays,
            sidebar_mode,
            rail_hints,
            rail_states,
            rail_letters,
            attention_req,
            palette_open,
            menu_open,
            menu_anchor,
            toasts,
        } = built;

        Self {
            window,
            device,
            queue,
            surface,
            config,
            grid,
            text,
            compositor,
            scale_factor,
            theme,
            ui,
            overlays,
            sidebar_mode,
            rail_hints,
            rail_states,
            rail_letters,
            rail_pick: false,
            clip_demo: false,
            attention_req,
            palette_open,
            menu_open,
            menu_anchor,
            toasts,
            ctrl: false,
            alt: false,
            meta: false,
            prefix_pending: false,
            zoom_mode: false,
            current_theme_idx: 0,
            ctl,
            cursor: Point::new(-1.0, -1.0),
            last_frame: Instant::now(),
            next_frame_in: None,
            force_full: true, // first frame paints everything
            layout_dirty: true,
            applied_size: WidgetSize::Normal,
            applied_zoom: ZOOM_DEFAULT,
            focus: FocusManager::new(),
            focus_ov: FocusManager::new(),
            shift: false,
            keymap: Keymap::with_defaults(),
        }
    }

}
