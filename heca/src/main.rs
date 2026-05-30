mod app_state;
mod chrome;
mod input;

use app_state::{AppState, SidebarState, DragState, InputMode};
use chrome::ChromeConfig;
use heca_config::theme::AppConfig;
use heca_core::pane::{PaneTree, SplitDirection};

use heca_renderer::primitive::PrimitiveRenderer;
use heca_renderer::text::TextRenderer;
use input::{KeyBindings, WmAction};
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::{WindowEvent, MouseButton, ElementState};

use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};

use winit::window::{Window, WindowId};

struct HecaApp {
    state: Option<Box<AppState>>,
    app_config: AppConfig,
    #[allow(dead_code)]
    bindings: KeyBindings,
}

impl HecaApp {
    fn new() -> Self {
        let app_config = AppConfig::load();
        let bindings = KeyBindings::load(&app_config);
        Self {
            state: None,
            app_config,
            bindings,
        }
    }

    async fn init_state(&mut self, event_loop: &ActiveEventLoop) -> Box<AppState> {
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
        text_renderer.set_font_family(&self.app_config.theme.font_family);
        text_renderer.set_screen_size(&queue, physical.width as f32 / scale_factor as f32, physical.height as f32 / scale_factor as f32);
        primitive_renderer.set_screen_size(&queue, physical.width as f32 / scale_factor as f32, physical.height as f32 / scale_factor as f32);

        // Initialize PaneTree: one editor pane and one floating terminal
        let mut panetree = PaneTree::new();
        let editor_id = panetree.add_pane("Editor", [0.118, 0.118, 0.180, 1.0]);
        let _term_id = panetree.add_pane("Terminal", [0.094, 0.094, 0.145, 1.0]);
        // Float the terminal centered, 60% of window width, 60% of height
        // Float rect is in LOGICAL pixels (relative to pane content area)
        // Content area = full window minus chrome
        let _log_w = physical.width as f32 / scale_factor as f32;
        let _log_h = physical.height as f32 / scale_factor as f32;

        // No auto-float — user creates floats via Ctrl+B + f

        Box::new(AppState {
            window,
            surface,
            device,
            queue,
            surface_config: config,
            primitive_renderer,
            text_renderer,
            panetree,
            theme: self.app_config.theme.clone(),
            scale_factor,
            needs_redraw: true,
            focused_pane: Some(editor_id),
            input_mode: InputMode::Normal,
            drag_state: DragState::None,
            sidebar: SidebarState {
                left_visible: true,
                left_width: 200.0,
                right_visible: true,
                right_width: 200.0,
            },
            active_tab: 0,
            tab_names: vec!["Main".to_string()],
            mouse_pos: (0.0, 0.0),
            modifiers: winit::keyboard::ModifiersState::default(),
        })
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
                state.surface.configure(&state.device, &state.surface_config);
                return;
            }
            Err(wgpu::SurfaceError::OutOfMemory) => std::process::exit(1),
            Err(e) => {
                eprintln!("Surface error: {:?}", e);
                return;
            }
        };

        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let phys_size = state.window.inner_size();
        let scale = state.scale_factor as f32;
        let w = phys_size.width as f32 / scale;
        let h = phys_size.height as f32 / scale;

        let theme = &state.theme;

        let chrome = ChromeConfig {
            tab_bar_height: 32.0,
            status_bar_height: 24.0,
            left_sidebar_width: if state.sidebar.left_visible {
                state.sidebar.left_width
            } else {
                32.0
            },
            right_sidebar_width: if state.sidebar.right_visible {
                state.sidebar.right_width
            } else {
                32.0
            },
        };
        let pane_area = chrome.content_rect(w, h);

        let mut encoder = state
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("render") });

        // Clear background
        let bg = theme.background.to_f32x4();
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: bg[0] as f64,
                        g: bg[1] as f64,
                        b: bg[2] as f64,
                        a: bg[3] as f64,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        // Chrome text: fixed readable size (theme.font_size is for terminal content)
        let chrome_text = 14.0f32;
        let pane_text = 16.0f32;

        // ── LEFT SIDEBAR ──
        let side_bg = if theme.name == "Catppuccin Mocha" {
            [0.067, 0.067, 0.106, 1.0]
        } else {
            [0.953, 0.957, 0.973, 1.0]
        };
        let sidebar_top = chrome.tab_bar_height;
        let sidebar_bottom = h - chrome.status_bar_height;
        let sidebar_h = sidebar_bottom - sidebar_top;
        state.primitive_renderer.draw_rect(0.0, sidebar_top, chrome.left_sidebar_width, sidebar_h, side_bg);
        state.primitive_renderer.draw_border(
            chrome.left_sidebar_width - 1.0, sidebar_top, 1.0, sidebar_h,
            theme.border.to_f32x4(), 1.0,
        );
        state.text_renderer.queue_text(
            "Sessions", 8.0, sidebar_top + 8.0, chrome_text, theme.foreground.to_f32x4(),
        );
        state.text_renderer.queue_text(
            "  (empty — Phase 4)", 8.0, sidebar_top + 8.0 + chrome_text * 2.2, chrome_text * 0.75,
            [theme.foreground.to_f32x4()[0], theme.foreground.to_f32x4()[1], theme.foreground.to_f32x4()[2], 0.5],
        );

        // ── RIGHT SIDEBAR ──
        let rsx = w - chrome.right_sidebar_width;
        state.primitive_renderer.draw_rect(rsx, sidebar_top, chrome.right_sidebar_width, sidebar_h, side_bg);
        state.primitive_renderer.draw_border(
            rsx, sidebar_top, 1.0, sidebar_h,
            theme.border.to_f32x4(), 1.0,
        );
        state.text_renderer.queue_text(
            "Details", rsx + 8.0, sidebar_top + 8.0, chrome_text, theme.foreground.to_f32x4(),
        );

        // ── TAB BAR ──
        let tb = &chrome;
        state.primitive_renderer.draw_rect(0.0, 0.0, w, tb.tab_bar_height, side_bg);
        for (i, tab_name) in state.tab_names.iter().enumerate() {
            let tab_x = 4.0 + i as f32 * 120.0;
            let tab_color = if i == state.active_tab {
                theme.accent.to_f32x4()
            } else {
                theme.border.to_f32x4()
            };
            state.primitive_renderer.draw_rect(tab_x, 2.0, 116.0, tb.tab_bar_height - 4.0, tab_color);
            let tab_text_y = (tb.tab_bar_height - chrome_text) / 2.0;
            state.text_renderer.queue_text(
                tab_name, tab_x + 4.0, tab_text_y, chrome_text, theme.foreground.to_f32x4(),
            );
        }

        // ── STATUS BAR ──
        let sb_y = h - tb.status_bar_height;
        state.primitive_renderer.draw_rect(0.0, sb_y, w, tb.status_bar_height, side_bg);
        let pane_count = state.panetree.visible_pane_count();
        let focus_title = state
            .focused_pane
            .and_then(|id| state.panetree.panes.iter().find(|p| p.id == id))
            .map(|p| p.title.as_str())
            .unwrap_or("—");
        let mode_str = match state.input_mode {
            InputMode::Normal => "NORMAL",
            InputMode::Prefix => "PREFIX",
        };
        let status = format!("{} panes | {} | {}", pane_count, focus_title, mode_str);
        let status_text_y = sb_y + (tb.status_bar_height - chrome_text) / 2.0;
        state.text_renderer.queue_text(
            &status, 8.0, status_text_y, chrome_text, theme.foreground.to_f32x4(),
        );

        // ── PANE CONTENT AREA ──
        let theme_border = theme.border.to_f32x4();
        let border_width = theme.border_width;
        let accent_color = theme.accent.to_f32x4();

        let (embedded, floats) = state.panetree.compute_rects(pane_area.w, pane_area.h);

        // ── Flush chrome before panes so pane text doesn't bleed over chrome ──
        state.primitive_renderer.render(&state.device, &view, &mut encoder);
        state.text_renderer.render(&state.device, &state.queue, &view, &mut encoder);

        // ── EMBEDDED PANES ──
        for (rect, pane) in &embedded {
            let px = pane_area.x + rect.x;
            let py = pane_area.y + rect.y;
            state.primitive_renderer.draw_rect(px, py, rect.w, rect.h, pane.background);
            let is_active = state.focused_pane == Some(pane.id);
            let bcolor = if is_active { accent_color } else {
                [theme_border[0], theme_border[1], theme_border[2], 0.5]
            };
            state.primitive_renderer.draw_border(px, py, rect.w, rect.h, bcolor, border_width);
            state.text_renderer.queue_text(
                &pane.title, px + 4.0, py + 4.0, pane_text, theme.foreground.to_f32x4(),
            );
        }
        state.primitive_renderer.render(&state.device, &view, &mut encoder);
        state.text_renderer.render(&state.device, &state.queue, &view, &mut encoder);

        // ── FLOATING PANES (back-to-front, each with own flush so text doesn't bleed) ──
        for (pane, rect) in &floats {
            let fx = pane_area.x + rect.x;
            let fy = pane_area.y + rect.y;
            let fbg = [0.192, 0.196, 0.267, 1.0];
            state.primitive_renderer.draw_rect(fx, fy, rect.w, rect.h, fbg);
            state.primitive_renderer.draw_border(fx, fy, rect.w, rect.h, accent_color, border_width * 2.0);
            // Title bar for float
            state.primitive_renderer.draw_rect(fx, fy, rect.w, 24.0, accent_color);
            state.text_renderer.queue_text(
                &pane.title, fx + 4.0, fy + 4.0, pane_text, [1.0, 1.0, 1.0, 1.0],
            );
            // Flush each float individually so lower floats' text never shows through
            state.primitive_renderer.render(&state.device, &view, &mut encoder);
            state.text_renderer.render(&state.device, &state.queue, &view, &mut encoder);
        }

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
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(phys) => {
                if phys.width > 0 && phys.height > 0 {
                    state.surface_config.width = phys.width;
                    state.surface_config.height = phys.height;
                    state.surface.configure(&state.device, &state.surface_config);
                    let log_w = phys.width as f32 / state.scale_factor as f32;
                    let log_h = phys.height as f32 / state.scale_factor as f32;
                    state.primitive_renderer.set_screen_size(&state.queue, log_w, log_h);
                    state.text_renderer.set_screen_size(&state.queue, log_w, log_h);
                    state.needs_redraw = true;
                }
            }
            WindowEvent::RedrawRequested => {
                state.needs_redraw = true;
                self.render();
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                state.scale_factor = scale_factor;
                state.text_renderer.set_scale_factor(scale_factor);
                state.needs_redraw = true;
            }
            WindowEvent::KeyboardInput {
                event,
                ..
            } => {
                if event.state != ElementState::Pressed {
                    return;
                }
                state.needs_redraw = true;

                let is_ctrl = state.modifiers.control_key();
                let _is_alt = state.modifiers.alt_key();
                let is_shift = state.modifiers.shift_key();

                let log_key = &event.logical_key;
                let key_text = log_key.to_text().unwrap_or("").to_string();
                let phys = event.physical_key;

                // Detect Ctrl+B by multiple methods:
                let is_prefix = key_text == "\u{2}"                         // macOS control char
                    || matches!(log_key, winit::keyboard::Key::Character(c) if c == "\u{2}")
                    || (is_ctrl && phys == winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyB));

                // Detect Ctrl+C, Ctrl+D etc. for future pane forwarding
                // Detect arrow keys via physical_key
                let phys_key = event.physical_key;

                match state.input_mode {
                    InputMode::Normal => {
                        // Direct bindings (no prefix needed)
                        let direct_action = resolve_action_core(&key_text, &phys_key, is_shift, &event.logical_key);
                        match direct_action {
                            Some(WmAction::SplitHorizontal) | Some(WmAction::SplitVertical)
                            | Some(WmAction::Float) | Some(WmAction::Scratchpad)
                            | Some(WmAction::Hide) | Some(WmAction::ClosePane)
                            | Some(WmAction::FocusLeft) | Some(WmAction::FocusRight)
                            | Some(WmAction::FocusUp) | Some(WmAction::FocusDown)
                            | Some(WmAction::NextPane) | Some(WmAction::PrevPane) => {
                                // These require prefix mode, skip in normal mode
                            }
                            Some(action) => {
                                let current = state.focused_pane;
                                execute_action(action, current, state);
                                return;
                            }
                            None => {}
                        }

                        if is_prefix {
                            state.input_mode = InputMode::Prefix;
                            state.needs_redraw = true;
                            return;
                        }
                    }
                    InputMode::Prefix => {
                        state.input_mode = InputMode::Normal;

                        if is_prefix {
                            return;
                        }

                        let action = resolve_action_core(&key_text, &phys_key, is_shift, &event.logical_key);
                        if let Some(act) = action {
                            execute_action(act, state.focused_pane, state);
                        }
                    }
                }
            }
            WindowEvent::ModifiersChanged(new_mods) => {
                state.modifiers = new_mods.state();
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.mouse_pos = (
                    position.x as f32 / state.scale_factor as f32,
                    position.y as f32 / state.scale_factor as f32,
                );
                state.needs_redraw = true;

                match state.drag_state {
                    DragState::Resizing { pane_id, dir, start_pos } => {
                        let phys = state.window.inner_size();
                        let win_w = phys.width as f32 / state.scale_factor as f32;
                        let win_h = phys.height as f32 / state.scale_factor as f32;
                        let mx = state.mouse_pos.0;
                        let my = state.mouse_pos.1;
                        if state.panetree.panes.iter().any(|p| p.id == pane_id) {
                            let delta = match dir {
                                SplitDirection::Horizontal => (mx - start_pos.0) / win_w,
                                SplitDirection::Vertical => (my - start_pos.1) / win_h,
                            };
                            state.panetree.resize(pane_id, delta * 0.1);
                            state.drag_state = DragState::Resizing {
                                pane_id,
                                dir,
                                start_pos: (mx, my),
                            };
                        }
                    }
                    DragState::MovingFloat { pane_id, offset, start_rect } => {
                        let mx = state.mouse_pos.0;
                        let my = state.mouse_pos.1;
                        // Recompute pane_area to convert screen -> content-area coords
                        let phys = state.window.inner_size();
                        let log_w = phys.width as f32 / state.scale_factor as f32;
                        let log_h = phys.height as f32 / state.scale_factor as f32;
                        let chrome = ChromeConfig {
                            tab_bar_height: 32.0,
                            status_bar_height: 24.0,
                            left_sidebar_width: if state.sidebar.left_visible { state.sidebar.left_width } else { 32.0 },
                            right_sidebar_width: if state.sidebar.right_visible { state.sidebar.right_width } else { 32.0 },
                        };
                        let pane_area = chrome.content_rect(log_w, log_h);

                        let new_abs_x = mx - offset.0;
                        let new_abs_y = my - offset.1;
                        let new_rect = heca_core::types::Rect::new(
                            new_abs_x - pane_area.x,
                            new_abs_y - pane_area.y,
                            start_rect.w,
                            start_rect.h,
                        );
                        // Clamp so float stays at least partly visible
                        let clamped = heca_core::types::Rect::new(
                            new_rect.x.clamp(-new_rect.w + 40.0, pane_area.w - 40.0),
                            new_rect.y.clamp(-new_rect.h + 40.0, pane_area.h - 40.0),
                            new_rect.w,
                            new_rect.h,
                        );
                        if let Some(entry) = state.panetree.floats.iter_mut().find(|(id, _)| *id == pane_id) {
                            entry.1 = clamped;
                        }
                        state.drag_state = DragState::MovingFloat {
                            pane_id,
                            offset,
                            start_rect: clamped,
                        };
                    }
                    DragState::ResizingFloat { pane_id, edge, start_mouse, start_rect } => {
                        let mx = state.mouse_pos.0;
                        let my = state.mouse_pos.1;
                        let dx = mx - start_mouse.0;
                        let dy = my - start_mouse.1;
                        let mut r = start_rect;
                        match edge {
                            app_state::FloatEdge::Left | app_state::FloatEdge::TopLeft | app_state::FloatEdge::BottomLeft => { r.x += dx; r.w -= dx; }
                            app_state::FloatEdge::Right | app_state::FloatEdge::TopRight | app_state::FloatEdge::BottomRight => { r.w += dx; }
                            _ => {}
                        }
                        match edge {
                            app_state::FloatEdge::Top | app_state::FloatEdge::TopLeft | app_state::FloatEdge::TopRight => { r.y += dy; r.h -= dy; }
                            app_state::FloatEdge::Bottom | app_state::FloatEdge::BottomLeft | app_state::FloatEdge::BottomRight => { r.h += dy; }
                            _ => {}
                        }
                        let min_size = 100.0;
                        if r.w >= min_size && r.h >= min_size {
                            if let Some(entry) = state.panetree.floats.iter_mut().find(|(id, _)| *id == pane_id) {
                                entry.1 = r;
                            }
                            state.drag_state = DragState::ResizingFloat { pane_id, edge, start_mouse: (mx, my), start_rect: r };
                        }
                    }
                    DragState::None => {}
                }
            }
            WindowEvent::MouseInput { state: button_state, button, .. } => {
                state.needs_redraw = true;
                let mouse_pos = state.mouse_pos;
                let phys = state.window.inner_size();
                let win_w = phys.width as f32 / state.scale_factor as f32;
                let win_h = phys.height as f32 / state.scale_factor as f32;
                let chrome = ChromeConfig {
                    tab_bar_height: 32.0,
                    status_bar_height: 24.0,
                    left_sidebar_width: if state.sidebar.left_visible { state.sidebar.left_width } else { 32.0 },
                    right_sidebar_width: if state.sidebar.right_visible { state.sidebar.right_width } else { 32.0 },
                };
                let pane_area = chrome.content_rect(win_w, win_h);

                if button == MouseButton::Left && button_state == ElementState::Pressed {
                    // Scope to end compute_rects borrow before mutating panetree
                    let float_click = {
                        let (_embedded, floats) = state.panetree.compute_rects(pane_area.w, pane_area.h);

                        // Hit test floating panes first (front-to-back)
                        let mut result: Option<(u64, Option<DragState>)> = None;
                        for (pane, rect) in floats.iter().rev() {
                            let abs_x = pane_area.x + rect.x;
                            let abs_y = pane_area.y + rect.y;
                            let in_rect = mouse_pos.0 >= abs_x && mouse_pos.0 <= abs_x + rect.w
                                && mouse_pos.1 >= abs_y && mouse_pos.1 <= abs_y + rect.h;
                            let edge = if in_rect {
                                let near_left = (mouse_pos.0 - abs_x).abs() < 6.0;
                                let near_right = (mouse_pos.0 - (abs_x + rect.w)).abs() < 6.0;
                                let near_top = (mouse_pos.1 - abs_y).abs() < 6.0;
                                let near_bottom = (mouse_pos.1 - (abs_y + rect.h)).abs() < 6.0;
                                match (near_left, near_right, near_top, near_bottom) {
                                    (true, _, true, _) => Some(app_state::FloatEdge::TopLeft),
                                    (_, true, true, _) => Some(app_state::FloatEdge::TopRight),
                                    (true, _, _, true) => Some(app_state::FloatEdge::BottomLeft),
                                    (_, true, _, true) => Some(app_state::FloatEdge::BottomRight),
                                    (true, _, _, _) => Some(app_state::FloatEdge::Left),
                                    (_, true, _, _) => Some(app_state::FloatEdge::Right),
                                    (_, _, true, _) => Some(app_state::FloatEdge::Top),
                                    (_, _, _, true) => Some(app_state::FloatEdge::Bottom),
                                    _ => None,
                                }
                            } else {
                                None
                            };

                            let on_title = in_rect && mouse_pos.1 >= abs_y && mouse_pos.1 <= abs_y + 24.0;

                            if let Some(e) = edge {
                                result = Some((pane.id, Some(DragState::ResizingFloat {
                                    pane_id: pane.id,
                                    edge: e,
                                    start_mouse: mouse_pos,
                                    start_rect: *rect,
                                })));
                                break;
                            } else if on_title {
                                let offset = (mouse_pos.0 - abs_x, mouse_pos.1 - abs_y);
                                result = Some((pane.id, Some(DragState::MovingFloat {
                                    pane_id: pane.id,
                                    offset,
                                    start_rect: *rect,
                                })));
                                break;
                            } else if in_rect {
                                result = Some((pane.id, None));
                                break;
                            }
                        }
                        result
                    };

                    // Apply float click after compute_rects borrow ends
                    if let Some((pane_id, ref drag)) = float_click {
                        state.focused_pane = Some(pane_id);
                        state.panetree.bring_float_to_front(pane_id);
                        if let Some(d) = drag {
                            state.drag_state = d.clone();
                        }
                    }

                    // Hit test embedded panes (skip if a float was clicked)
                    if float_click.is_none() {
                        let (embedded, _) = state.panetree.compute_rects(pane_area.w, pane_area.h);
                        for (rect, pane) in &embedded {
                            let px = pane_area.x + rect.x;
                            let py = pane_area.y + rect.y;
                            if mouse_pos.0 >= px && mouse_pos.0 <= px + rect.w
                                && mouse_pos.1 >= py && mouse_pos.1 <= py + rect.h
                            {
                                state.focused_pane = Some(pane.id);
                                // Check if click is near a border to start resize
                                let near_left = (mouse_pos.0 - px).abs() < 4.0;
                                let near_right = (mouse_pos.0 - (px + rect.w)).abs() < 4.0;
                                let near_top = (mouse_pos.1 - py).abs() < 4.0;
                                let near_bottom = (mouse_pos.1 - (py + rect.h)).abs() < 4.0;
                                if near_left || near_right {
                                    state.drag_state = DragState::Resizing {
                                        pane_id: pane.id,
                                        dir: SplitDirection::Horizontal,
                                        start_pos: mouse_pos,
                                    };
                                } else if near_top || near_bottom {
                                    state.drag_state = DragState::Resizing {
                                        pane_id: pane.id,
                                        dir: SplitDirection::Vertical,
                                        start_pos: mouse_pos,
                                    };
                                }
                                break;
                            }
                        }
                    }
                }

                if button == MouseButton::Left && button_state == ElementState::Released {
                    state.drag_state = DragState::None;
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(ref state) = self.state {
            if state.needs_redraw {
                state.window.request_redraw();
            }
        }
    }
}

fn execute_action(action: WmAction, current: Option<u64>, state: &mut AppState) {
    match action {
        WmAction::FocusLeft | WmAction::FocusRight => {
            if let Some(id) = current {
                state.focused_pane = state.panetree.find_neighbor(id, SplitDirection::Horizontal);
            }
        }
        WmAction::FocusUp | WmAction::FocusDown => {
            if let Some(id) = current {
                state.focused_pane = state.panetree.find_neighbor(id, SplitDirection::Vertical);
            }
        }
        WmAction::SplitHorizontal => {
            if let Some(id) = current {
                if let Some(new_id) = state.panetree.split(id, SplitDirection::Horizontal) {
                    state.focused_pane = Some(new_id);
                }
            }
        }
        WmAction::SplitVertical => {
            if let Some(id) = current {
                if let Some(new_id) = state.panetree.split(id, SplitDirection::Vertical) {
                    state.focused_pane = Some(new_id);
                }
            }
        }
        WmAction::Float => {
            if let Some(id) = current {
                state.panetree.toggle_float(id, None);
            }
        }
        WmAction::Scratchpad => {
            if let Some(id) = current {
                state.panetree.toggle_scratchpad(id);
            }
        }
        WmAction::Hide => {
            if let Some(id) = current {
                state.panetree.hide(id);
                state.focused_pane = None;
            }
        }
        WmAction::ClosePane => {
            if let Some(id) = current {
                state.panetree.remove(id);
                state.focused_pane = None;
            }
        }
        WmAction::TabNext => {
            if !state.tab_names.is_empty() {
                state.active_tab = (state.active_tab + 1) % state.tab_names.len();
            }
        }
        WmAction::TabPrev => {
            if !state.tab_names.is_empty() {
                state.active_tab = (state.active_tab + state.tab_names.len() - 1) % state.tab_names.len();
            }
        }
        WmAction::ResizeLeft | WmAction::ResizeUp => {
            if let Some(id) = current {
                state.panetree.resize(id, -0.05);
            }
        }
        WmAction::ResizeRight | WmAction::ResizeDown => {
            if let Some(id) = current {
                state.panetree.resize(id, 0.05);
            }
        }
        WmAction::SidebarLeft => {
            state.sidebar.left_visible = !state.sidebar.left_visible;
        }
        WmAction::SidebarRight => {
            state.sidebar.right_visible = !state.sidebar.right_visible;
        }
        WmAction::NextPane => {
            state.focused_pane = state.panetree.cycle_focus(state.focused_pane, 1);
        }
        WmAction::PrevPane => {
            state.focused_pane = state.panetree.cycle_focus(state.focused_pane, -1);
        }
    }
}

/// Core key-to-action resolution independent of modifier state.
fn resolve_action_core(
    key_text: &str,
    phys_key: &winit::keyboard::PhysicalKey,
    shift: bool,
    logical_key: &winit::keyboard::Key,
) -> Option<WmAction> {
    // Arrow keys
    if let winit::keyboard::PhysicalKey::Code(code) = phys_key {
        match code {
            winit::keyboard::KeyCode::ArrowLeft => return Some(WmAction::FocusLeft),
            winit::keyboard::KeyCode::ArrowRight => return Some(WmAction::FocusRight),
            winit::keyboard::KeyCode::ArrowUp => return Some(WmAction::FocusUp),
            winit::keyboard::KeyCode::ArrowDown => return Some(WmAction::FocusDown),
            _ => {}
        }
    }

    // Named keys
    if let winit::keyboard::Key::Named(named) = logical_key {
        match named {
            winit::keyboard::NamedKey::Space => return Some(WmAction::SidebarLeft),
            _ => {}
        }
    }

    // Text keys (single characters)
    match key_text {
        "h" if !shift => return Some(WmAction::FocusLeft),
        "j" if !shift => return Some(WmAction::FocusDown),
        "k" if !shift => return Some(WmAction::FocusUp),
        "l" if !shift => return Some(WmAction::FocusRight),
        "-" => return Some(WmAction::SplitHorizontal),
        "v" => return Some(WmAction::SplitVertical),
        "f" => return Some(WmAction::Float),
        "s" => return Some(WmAction::Scratchpad),
        "z" => return Some(WmAction::Hide),
        "x" => return Some(WmAction::ClosePane),
        "]" => return Some(WmAction::TabNext),
        "[" => return Some(WmAction::TabPrev),
        "n" => return Some(WmAction::NextPane),
        "p" => return Some(WmAction::PrevPane),
        "H" => return Some(WmAction::ResizeLeft),
        "L" => return Some(WmAction::ResizeRight),
        "K" => return Some(WmAction::ResizeUp),
        "J" => return Some(WmAction::ResizeDown),
        _ => {}
    }

    None
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = HecaApp::new();
    event_loop.run_app(&mut app).unwrap();
}
