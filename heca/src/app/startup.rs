//! App startup and renderer/session initialization helpers.
//!
//! This module owns first-launch wiring so `main.rs` can focus on lifecycle
//! control flow rather than GPU/window/session bootstrapping details.

use crate::app_state::{self, AppState, InputMode, SidebarState};
use crate::chrome::{ChromeConfig, DEFAULT_TAB_BAR_HEIGHT, DEFAULT_STATUS_BAR_HEIGHT};
use crate::keymap;
use crate::pane_name;
use crate::sidebar::SidebarTree;
use heca_config::theme::AppConfig;
use heca_core::backend::{FakeBackend, PaneBackend};
use heca_core::layout::{Pane as LayoutPane, PaneId, Session};
use heca_renderer::primitive::PrimitiveRenderer;
use heca_renderer::text::TextRenderer;
use std::collections::HashMap;
use std::sync::Arc;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

pub(crate) async fn init_state(
    app_config: &AppConfig,
    event_loop: &ActiveEventLoop,
) -> Box<AppState> {
    let window_attrs = Window::default_attributes()
        .with_title("heca")
        .with_inner_size(winit::dpi::LogicalSize::new(
            app_config.config.settings.window_width as f64,
            app_config.config.settings.window_height as f64,
        ));
    let window = Arc::new(
        event_loop
            .create_window(window_attrs)
            .expect("Failed to create window"),
    );
    let scale_factor = window.scale_factor();

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let surface = instance
        .create_surface(window.clone())
        .expect("Failed to create surface");

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })
        .await
        .expect("Failed to find an appropriate adapter");

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("heca_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
        })
        .await
        .expect("Failed to create device");

    let surface_caps = surface.get_capabilities(&adapter);
    let surface_format = surface_caps
        .formats
        .iter()
        .find(|f| f.is_srgb())
        .copied()
        .unwrap_or(surface_caps.formats[0]);

    let physical = window.inner_size();
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width: physical.width.max(1),
        height: physical.height.max(1),
        present_mode: wgpu::PresentMode::AutoVsync,
        desired_maximum_frame_latency: 2,
        alpha_mode: surface_caps.alpha_modes[0],
        view_formats: vec![],
    };
    surface.configure(&device, &config);

    let mut primitive_renderer = PrimitiveRenderer::new(&device, surface_format);
    let mut text_renderer = TextRenderer::new(&device, surface_format);
    text_renderer.set_scale_factor(scale_factor);
    text_renderer.set_font_family(&app_config.theme.font_family);
    text_renderer.set_screen_size(
        &queue,
        physical.width as f32 / scale_factor as f32,
        physical.height as f32 / scale_factor as f32,
    );
    primitive_renderer.set_screen_size(
        &queue,
        physical.width as f32 / scale_factor as f32,
        physical.height as f32 / scale_factor as f32,
    );

    let chrome = ChromeConfig {
        tab_bar_height: DEFAULT_TAB_BAR_HEIGHT,
        status_bar_height: DEFAULT_STATUS_BAR_HEIGHT,
        left_sidebar_width: 200.0,
        right_sidebar_width: 200.0,
    };
    let log_w = physical.width as f32 / scale_factor as f32;
    let log_h = physical.height as f32 / scale_factor as f32;
    let pane_area = chrome.content_rect(log_w, log_h);

    let viewport_size = heca_core::layout::types::Size::new(pane_area.w as f64, pane_area.h as f64);
    let layout_options = heca_core::layout::types::LayoutOptions {
        always_center_single_column: app_config.config.settings.always_center_single_column,
        ..Default::default()
    };
    let mut session = Session::new(
        heca_core::layout::types::SessionId(1),
        viewport_size,
        scale_factor,
        layout_options,
    );

    let fake_pane = LayoutPane::new(PaneId(1), pane_name(1));
    let pane_id = fake_pane.id.0;
    session.add_pane(fake_pane, None, true);

    let mut backends: HashMap<u64, Box<dyn PaneBackend>> = HashMap::new();
    backends.insert(pane_id, Box::new(FakeBackend::new(80, 24)));

    let ws_count = session.workspaces.len();
    let mut sidebar_tree = SidebarTree::new();
    sidebar_tree.sync_from_session(&session, None, Some(pane_id), &vec![None; ws_count]);

    Box::new(AppState {
        window,
        surface,
        device,
        queue,
        surface_config: config,
        primitive_renderer,
        text_renderer,
        session,
        backends,
        theme: app_config.theme.clone(),
        scale_factor,
        needs_redraw: true,
        focused_pane: Some(pane_id),
        input_mode: InputMode::Normal,
        sidebar: SidebarState {
            left_visible: true,
            left_width: 200.0,
            right_visible: true,
            right_width: 200.0,
        },
        active_tab: 0,
        sidebar_tree,
        tab_names: vec!["Main".to_string()],
        mouse: app_state::MouseState::new(),
        modifiers: winit::keyboard::ModifiersState::default(),
        last_focused: None,
        last_visited_ws_idx: None,
        last_visited_pane_per_ws: vec![None; ws_count],
        mouse_enabled: app_config.config.settings.mouse,
        auto_scroll_edge: app_config.config.settings.auto_scroll_edge,
        interactive_move_modifier: app_config.config.settings.interactive_move_modifier,
        prefix_entered_at: None,
        prefix_combo: keymap::KeyCombo::parse(&app_config.config.keys.prefix),
        pending_reload: false,
    })
}
