//! App startup and renderer/session initialization helpers.
//!
//! This module owns first-launch wiring so `main.rs` can focus on lifecycle
//! control flow rather than GPU/window/session bootstrapping details.

use crate::app::backend_factory::{create_terminal_backend, estimate_terminal_grid};
use crate::app::backend_store::BackendStore;
use crate::app::terminal_metrics::resolve_terminal_cell_size;
use crate::app_state::{self, AppState, InputMode};
use crate::chrome::{ChromeConfig, DEFAULT_STATUS_BAR_HEIGHT, DEFAULT_TAB_BAR_HEIGHT};
use crate::keymap;
use crate::pane_name;
use heca_config::theme::AppConfig;
use heca_core::layout::{Pane as LayoutPane, PaneId, Session};
use heca_grid_ui::install_frame_request;
use heca_renderer::backdrop::Backdrop;
use heca_renderer::background::BackgroundLayer;
use heca_renderer::blur::Blur;
use heca_renderer::composite::Compositor;
use heca_renderer::grid::GridRenderer;
use heca_renderer::primitive::PrimitiveRenderer;
use heca_renderer::text::TextRenderer;
use std::sync::Arc;
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::window::Window;

fn add_initial_pane(session: &mut Session) -> PaneId {
    let pane_id = PaneId(session.next_id());
    let pane = LayoutPane::new(pane_id, pane_name(pane_id));
    session.add_pane(pane, None, true);
    pane_id
}

/// Pick a surface composite-alpha mode. When transparency is requested, prefer a
/// transparency-capable mode (premultiplied, matching the renderers' premultiplied
/// blending, then postmultiplied). Otherwise keep today's opaque behavior.
fn choose_alpha_mode(
    modes: &[wgpu::CompositeAlphaMode],
    transparent: bool,
) -> wgpu::CompositeAlphaMode {
    use wgpu::CompositeAlphaMode::*;
    if transparent {
        for pref in [PreMultiplied, PostMultiplied] {
            if modes.contains(&pref) {
                return pref;
            }
        }
    } else if modes.contains(&Opaque) {
        return Opaque;
    }
    modes.first().copied().unwrap_or(Auto)
}

/// Apply OS backdrop blur / vibrancy once after window creation, when enabled.
/// macOS = `NSVisualEffectView`; Windows = acrylic; Linux/other = no-op (the
/// effect is not portably available). Needs a transparent window (F2b) to show.
pub(crate) fn apply_window_vibrancy(
    window: &Window,
    appearance: &heca_config::appearance::AppearanceConfig,
) {
    let Some(vibrancy) = appearance.os_vibrancy() else {
        return;
    };
    #[cfg(target_os = "macos")]
    apply_macos_vibrancy(window, vibrancy);
    #[cfg(target_os = "windows")]
    {
        let _ = (
            vibrancy,
            window_vibrancy::apply_acrylic(window, Some((18, 18, 18, 125))),
        );
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (window, vibrancy); // no portable backdrop blur on Linux/other
    }
}

/// macOS backdrop blur done right. window-vibrancy adds the `NSVisualEffectView`
/// as a subview of winit's content view, which draws *over* wgpu's metal layer
/// and washes out the content. Reparenting the content view panics winit (it
/// owns its content view). So instead we insert the effect view as a **sibling
/// BEHIND** winit's content view, in the window's frame view (`superview`):
/// winit's view is left untouched, and where the transparent metal surface shows
/// through, the frosted effect view behind it is revealed.
#[cfg(target_os = "macos")]
fn apply_macos_vibrancy(window: &Window, vibrancy: heca_config::appearance::Vibrancy) {
    use heca_config::appearance::Vibrancy;
    use objc2_app_kit::{
        NSAutoresizingMaskOptions, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
        NSVisualEffectState, NSVisualEffectView, NSWindowOrderingMode,
    };
    use objc2_foundation::MainThreadMarker;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let material = match vibrancy {
        Vibrancy::None => return,
        Vibrancy::Sidebar => NSVisualEffectMaterial::Sidebar,
        Vibrancy::HudWindow => NSVisualEffectMaterial::HUDWindow,
        Vibrancy::UnderWindowBackground => NSVisualEffectMaterial::UnderWindowBackground,
        Vibrancy::Popover => NSVisualEffectMaterial::Popover,
        Vibrancy::Menu => NSVisualEffectMaterial::Menu,
        Vibrancy::FullScreenUi => NSVisualEffectMaterial::FullScreenUI,
        Vibrancy::WindowBackground => NSVisualEffectMaterial::WindowBackground,
    };

    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::AppKit(h) = handle.as_raw() else {
        return;
    };
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let fill = NSAutoresizingMaskOptions::NSViewWidthSizable
        | NSAutoresizingMaskOptions::NSViewHeightSizable;

    // SAFETY: `h.ns_view` is winit's live content NSView. We only READ it
    // (superview/frame) and add a sibling behind it — we never reparent or mutate
    // winit's view, so winit's ownership is intact. Main thread (init/event loop).
    unsafe {
        let content_view: &NSView = h.ns_view.cast().as_ref();
        let Some(frame_view) = content_view.superview() else {
            return;
        };
        let frame = content_view.frame();

        let effect = NSVisualEffectView::initWithFrame(mtm.alloc(), frame);
        effect.setMaterial(material);
        effect.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
        effect.setState(NSVisualEffectState::Active);
        effect.setAutoresizingMask(fill);
        frame_view.addSubview_positioned_relativeTo(
            &effect,
            NSWindowOrderingMode::NSWindowBelow,
            Some(content_view),
        );
    }
}

pub(crate) async fn init_state(
    app_config: &AppConfig,
    event_loop: &ActiveEventLoop,
    event_proxy: EventLoopProxy<crate::app::events::AppEvent>,
) -> Box<AppState> {
    let redraw_proxy = event_proxy.clone();
    install_frame_request(move || {
        let _ = redraw_proxy.send_event(crate::app::events::AppEvent::RequestRedraw);
    });

    let appearance = app_config.config.appearance.clone();
    let window_attrs = Window::default_attributes()
        .with_title("heca")
        .with_inner_size(winit::dpi::LogicalSize::new(
            app_config.config.settings.window_width as f64,
            app_config.config.settings.window_height as f64,
        ))
        .with_transparent(appearance.is_transparent());
    let window = Arc::new(
        event_loop
            .create_window(window_attrs)
            .expect("Failed to create window"),
    );
    apply_window_vibrancy(window.as_ref(), &appearance);
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
        alpha_mode: choose_alpha_mode(&surface_caps.alpha_modes, appearance.is_transparent()),
        view_formats: vec![],
    };
    surface.configure(&device, &config);

    let mut primitive_renderer = PrimitiveRenderer::new(&device, surface_format);
    let image_renderer = heca_renderer::image::ImageRenderer::new(&device, surface_format);
    let mut text_renderer = TextRenderer::new(&device, surface_format);
    text_renderer.set_scale_factor(scale_factor);
    text_renderer.set_target_size(physical.width, physical.height);
    text_renderer.set_font_family(app_config.config.font.family.ui_normal());
    text_renderer.set_screen_size(
        &queue,
        physical.width as f32 / scale_factor as f32,
        physical.height as f32 / scale_factor as f32,
    );
    let terminal_cell_size =
        resolve_terminal_cell_size(&mut text_renderer, &app_config.config.font);
    primitive_renderer.set_screen_size(
        &queue,
        physical.width as f32 / scale_factor as f32,
        physical.height as f32 / scale_factor as f32,
    );
    let mut grid_renderer = GridRenderer::new(&device, surface_format);
    grid_renderer.set_scale_factor(scale_factor);
    grid_renderer.set_target_size(physical.width, physical.height);
    grid_renderer.set_screen_size(
        &queue,
        physical.width as f32 / scale_factor as f32,
        physical.height as f32 / scale_factor as f32,
    );
    let compositor = Compositor::new(&device, surface_format, physical.width, physical.height);
    let blur = Blur::new(&device, surface_format, physical.width, physical.height);
    let backdrop = Backdrop::new(&device, surface_format);
    let background = BackgroundLayer::new(&device, surface_format, physical.width, physical.height);

    let chrome = ChromeConfig {
        tab_bar_height: if app_config.config.settings.show_top_bar {
            DEFAULT_TAB_BAR_HEIGHT
        } else {
            0.0
        },
        status_bar_height: if app_config.config.settings.show_bottom_bar {
            DEFAULT_STATUS_BAR_HEIGHT
        } else {
            0.0
        },
        left_sidebar_width: crate::chrome::DEFAULT_SIDEBAR_WIDTH,
        right_sidebar_width: crate::chrome::DEFAULT_SIDEBAR_WIDTH,
        sidebar_gap: app_config
            .config
            .appearance
            .effective_sidebar_gap(&app_config.theme),
    };
    let log_w = physical.width as f32 / scale_factor as f32;
    let log_h = physical.height as f32 / scale_factor as f32;
    let pane_area = chrome.content_rect(log_w, log_h);

    let viewport_size = heca_core::layout::types::Size::new(pane_area.size.w, pane_area.size.h);
    let layout_options = heca_core::layout::types::LayoutOptions {
        gaps: app_config
            .config
            .appearance
            .effective_pane_gap(&app_config.theme) as f64,
        always_center_single_column: app_config.config.settings.always_center_single_column,
        ..Default::default()
    };
    let mut session = Session::new(
        heca_core::layout::types::SessionId(1),
        viewport_size,
        scale_factor,
        layout_options,
    );

    let pane_id = add_initial_pane(&mut session);

    let mut backends = BackendStore::new();
    let (initial_cols, initial_rows) =
        estimate_terminal_grid(pane_area.size.w, pane_area.size.h, terminal_cell_size);
    backends.insert_for_pane(
        pane_id,
        create_terminal_backend(
            initial_cols,
            initial_rows,
            &app_config.theme,
            terminal_cell_size,
            Some(&event_proxy),
            app_config.config.settings.shell_integration,
            app_config.config.settings.terminal_scrollback_lines,
            app_config.config.settings.terminal_scroll_animations,
        ),
    );

    let ws_count = session.workspaces.len();

    let terminal_layer_scratch =
        app_state::RetainedTerminalScratch::new(&device, config.format, 1, 1);

    // Region visibility/width lives in chrome_state (was SidebarState). Build it
    // first so the ChromeHost can share its event bus. plugin-02: the host is
    // wired but empty — first-party providers register in plugin-03.
    // Region *mode* (Expanded/Hidden→rail) is the runtime expand/collapse state.
    // Config's `show_*_sidebar` is a separate hard "mounted" gate applied at render
    // time (`AppState::{left,right}_sidebar_width`), so the runtime rail toggle stays
    // independent of whether the region is configured on at all.
    let chrome_state = crate::chrome::SharedChromeState::new(
        app_config.config.appearance.effective_sidebar_width(),
        true,
        app_config.config.appearance.effective_sidebar_width(),
        true,
    );
    let mut chrome_host = crate::chrome::ChromeHost::new(chrome_state.events());
    // Register the first built-in provider, seating the `workspaces` container in
    // `LeftSidebar`. This is not bookkeeping: the render path builds each region's body
    // from whatever the host has seated in it (`chrome::build_region_content`), so this
    // registration is *why* there is a workspace tree in the sidebar at all. Move the
    // container to the right region and its UI goes with it.
    chrome_host.register(Box::new(crate::providers::WorkspacesContainerProvider::new()));
    // A **second placement** of the same container, in the right sidebar (F003/P085/T359, user
    // 2026-07-30). Not scaffolding: with one dock on screen none of this phase is observable — not
    // a letter per dock, not focus moving between them, not "the focused one answers and every
    // other declines", not two cursors, not two scroll positions. And a terminal has no horizontal
    // scroll at all, so the horizontal keys have nothing to act on without it.
    //
    // It is also the only thing that exercises the kind/mount split for real: same content, same
    // bindings, separate cursor / scroll / focus, because those are keyed by mount id.
    chrome_host.register(Box::new(
        crate::providers::WorkspacesContainerProvider::placed(
            "workspaces.right",
            crate::chrome::RegionId::RightSidebar,
        ),
    ));

    Box::new(AppState {
        window,
        event_proxy,
        surface,
        device,
        queue,
        surface_config: config,
        primitive_renderer,
        text_renderer,
        image_renderer,
        grid_renderer,
        compositor,
        terminal_layers: std::collections::HashMap::new(),
        terminal_layer_scratch,
        blur,
        backdrop,
        background,
        git_runtime_cache: crate::app::git_monitor::GitRuntimeCache::default(),
        session,
        backends,
        theme: app_config.theme.clone(),
        programs: app_config.config.programs.clone(),
        appearance,
        font_config: app_config.config.font.clone(),
        terminal_cell_size,
        has_animated_images: false,
        app_font_zoom: 0.0,
        pane_font_zoom: std::collections::HashMap::new(),
        pane_cell_override: std::collections::HashMap::new(),
        scale_factor,
        needs_redraw: true,
        focused_pane: Some(pane_id),
        input_mode: InputMode::Normal,
        overlay_origin_mode: None,
        chrome_tree: None,
        pane_headers: std::collections::HashMap::new(),
        hint_targets: crate::chrome::HintTargetRegistry::default(),
        layers: crate::chrome::LayerRegistry::default(),
        overlays: crate::chrome::OverlayHost::default(),
        pane_viewport_widgets: std::collections::HashMap::new(),
        action_shortcuts: crate::chrome::ActionShortcuts::from_config(&app_config.config),
        action_catalog: crate::actions::ActionCatalog::with_builtins(),
        context_menu_registry: crate::chrome::ContextMenuRegistry::with_builtins(),
        pending_context: None,
        // Region visibility/width now lives in chrome_state (was SidebarState).
        chrome_state,
        chrome_host,
        mouse: app_state::MouseState::new(),
        modifiers: winit::keyboard::ModifiersState::default(),
        selection: app_state::SelectionState::new(),
        bell_flash_until: None,
        searches: std::collections::BTreeMap::new(),
        last_focused: None,
        last_visited_ws_idx: None,
        last_visited_pane_per_ws: vec![None; ws_count],
        mouse_enabled: app_config.config.settings.mouse,
        auto_scroll_edge: app_config.config.settings.auto_scroll_edge,
        shell_integration_enabled: app_config.config.settings.shell_integration,
        pane_renamed_add_process_name: app_config.config.settings.pane_renamed_add_process_name,
        pane_show_cwd: app_config.config.settings.pane_show_cwd,
        terminal_scrollback_lines: app_config.config.settings.terminal_scrollback_lines,
        terminal_mouse_enabled: app_config.config.settings.terminal_mouse,
        terminal_wheel_scroll_lines: app_config.config.settings.terminal_wheel_scroll_lines,
        terminal_font_zoom_step: app_config.config.settings.terminal_font_zoom_step,
        mouse_wheel_change_font_size: app_config.config.settings.mouse_wheel_change_font_size,
        terminal_scroll_animations_enabled: app_config.config.settings.terminal_scroll_animations,
        show_left_sidebar: app_config.config.settings.show_left_sidebar,
        show_right_sidebar: app_config.config.settings.show_right_sidebar,
        show_top_bar: app_config.config.settings.show_top_bar,
        show_bottom_bar: app_config.config.settings.show_bottom_bar,
        confirm: app_config.config.confirm.clone(),
        interactive_move_modifier: app_config.config.settings.interactive_move_modifier,
        prefix_entered_at: None,
        prefix_combo: keymap::KeyCombo::parse(&app_config.config.keys.prefix),
        widget_keymap: crate::app::registry::build_widget_keymap(&app_config.config),
        pending_reload: false,
        window_focused: true,
        current_cursor: winit::window::CursorIcon::Default,
    })
}

#[cfg(test)]
mod tests {
    use super::add_initial_pane;
    use heca_core::layout::{
        Session,
        types::{LayoutOptions, SessionId, Size},
    };

    #[test]
    fn initial_pane_consumes_session_id_counter() {
        let mut session = Session::new(
            SessionId(1),
            Size::new(1280.0, 800.0),
            1.0,
            LayoutOptions::default(),
        );

        let first = add_initial_pane(&mut session);
        let second = session.next_id();

        assert_eq!(second, first.0 + 1);
    }
}
