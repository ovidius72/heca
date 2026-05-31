mod app_state;
mod chrome;
mod input;
mod sidebar;

use sidebar::SidebarTree;

use app_state::{AppState, SidebarItemState, SidebarState, DragState, InputMode, RenameTarget};
use chrome::ChromeConfig;
use heca_config::theme::AppConfig;
use heca_core::backend::{BackendRenderData, PaneBackend, FakeBackend};
use heca_core::layout::{Session, Column, Pane as LayoutPane, ColumnId, PaneId, ColumnWidth, ViewOffset};
use heca_core::layout::types::{Rectangle, Point, Size};
use heca_core::layout::animation::{Animation, AnimationConfig};
use heca_core::pane::{SplitDirection, GeoDir};

use heca_renderer::primitive::PrimitiveRenderer;
use heca_renderer::text::TextRenderer;
use input::{KeyBindings, WmAction};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::{WindowEvent, MouseButton, ElementState};
use winit::keyboard::{Key, NamedKey};

use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};

use winit::window::{Window, WindowId};

/// Render a backend's content into a pane rectangle.
fn render_backend_data(
    data: &BackendRenderData,
    px: f32,
    py: f32,
    _pw: f32,
    _ph: f32,
    text_renderer: &mut TextRenderer,
    primitive_renderer: &mut PrimitiveRenderer,
    _theme: &heca_config::theme::Theme,
) {
    match data {
        BackendRenderData::Terminal {
            lines,
            cursor_col,
            cursor_row,
        } => {
            let cell_h = 14.0f32;
            let cell_w = 8.4f32; // ~0.6 * cell_h for monospace

            // Background
            primitive_renderer.draw_rect(px, py, _pw, _ph, [0.0, 0.0, 0.0, 1.0]);

            // Text
            for (row, line) in lines.iter().enumerate() {
                let y = py + row as f32 * cell_h;
                let mut current_text = String::new();
                let mut current_fg = [1.0f32; 4];
                let mut start_col = 0usize;

                for (col, cell) in line.cells.iter().enumerate() {
                    if cell.c == ' ' || cell.c == '\0' {
                        if !current_text.is_empty() {
                            let x = px + start_col as f32 * cell_w;
                            text_renderer.queue_text(&current_text, x, y, cell_h, current_fg);
                            current_text.clear();
                        }
                        start_col = col + 1;
                        continue;
                    }

                    if col > start_col && cell.fg != current_fg {
                        if !current_text.is_empty() {
                            let x = px + start_col as f32 * cell_w;
                            text_renderer.queue_text(&current_text, x, y, cell_h, current_fg);
                            current_text.clear();
                        }
                        current_fg = cell.fg;
                        start_col = col;
                    }

                    current_text.push(cell.c);
                }

                if !current_text.is_empty() {
                    let x = px + start_col as f32 * cell_w;
                    text_renderer.queue_text(&current_text, x, y, cell_h, current_fg);
                }
            }

            // Cursor
            let cursor_x = px + *cursor_col as f32 * cell_w;
            let cursor_y = py + *cursor_row as f32 * cell_h;
            primitive_renderer.draw_rect(cursor_x, cursor_y, cell_w, cell_h, [1.0, 1.0, 1.0, 0.7]);
        }
        _ => {}
    }
}

/// Distinct pane names so you can visually identify what's moving.
const PANE_NAMES: &[&str] = &[
    "Red", "Green", "Blue", "Yellow", "Cyan", "Magenta",
    "Orange", "Purple", "Lime", "Pink", "Teal", "Coral",
];

fn pane_name(id: u64) -> String {
    PANE_NAMES.get((id as usize).saturating_sub(1) % PANE_NAMES.len())
        .unwrap_or(&"?")
        .to_string()
}

/// Convert a winit key event to terminal input bytes.
fn winit_key_to_terminal_input(
    key: &winit::keyboard::Key,
    text: &str,
    ctrl: bool,
) -> Vec<u8> {
    use winit::keyboard::NamedKey;

    // Ctrl+letter -> control character
    if ctrl && text.len() == 1 {
        let c = text.as_bytes()[0];
        if c >= b'a' && c <= b'z' {
            return vec![c - b'a' + 1];
        }
    }

    match key {
        Key::Named(NamedKey::Enter) => vec![b'\r'],
        Key::Named(NamedKey::Backspace) => vec![0x7f],
        Key::Named(NamedKey::Tab) => vec![b'\t'],
        Key::Named(NamedKey::Escape) => vec![0x1b],
        Key::Named(NamedKey::ArrowUp) => b"\x1b[A".to_vec(),
        Key::Named(NamedKey::ArrowDown) => b"\x1b[B".to_vec(),
        Key::Named(NamedKey::ArrowRight) => b"\x1b[C".to_vec(),
        Key::Named(NamedKey::ArrowLeft) => b"\x1b[D".to_vec(),
        Key::Named(NamedKey::Home) => b"\x1b[H".to_vec(),
        Key::Named(NamedKey::End) => b"\x1b[F".to_vec(),
        Key::Named(NamedKey::PageUp) => b"\x1b[5~".to_vec(),
        Key::Named(NamedKey::PageDown) => b"\x1b[6~".to_vec(),
        Key::Named(NamedKey::Delete) => b"\x1b[3~".to_vec(),
        Key::Named(NamedKey::Space) => vec![b' '],
        Key::Character(c) => c.as_bytes().to_vec(),
        _ => vec![],
    }
}

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

        // Compute chrome/content area size FIRST so session uses correct working area
        let chrome = ChromeConfig {
            tab_bar_height: 32.0,
            status_bar_height: 24.0,
            left_sidebar_width: 200.0,
            right_sidebar_width: 200.0,
        };
        let log_w = physical.width as f32 / scale_factor as f32;
        let log_h = physical.height as f32 / scale_factor as f32;
        let pane_area = chrome.content_rect(log_w, log_h);

        // Initialize NIRI Session with content area size (not full window)
        let viewport_size = heca_core::layout::types::Size::new(
            pane_area.w as f64,
            pane_area.h as f64,
        );
        let mut session = Session::new(
            heca_core::layout::types::SessionId(1),
            viewport_size,
            scale_factor,
        );

        // Create fake pane and add to workspace
        let fake_pane = LayoutPane::new(PaneId(1), &pane_name(1));
        let pane_id = fake_pane.id.0;
        session.add_pane(fake_pane, None, true);

        // Create fake backend (no PTY overhead for layout testing)
        let mut backends: HashMap<u64, Box<dyn PaneBackend>> = HashMap::new();
        backends.insert(pane_id, Box::new(FakeBackend::new(80, 24)));

        let ws_count = session.workspaces.len();
        let mut sidebar_tree = SidebarTree::new();
        sidebar_tree.rebuild(&session, None, Some(pane_id));

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
            theme: self.app_config.theme.clone(),
            scale_factor,
            needs_redraw: true,
            focused_pane: Some(pane_id),
            input_mode: InputMode::Normal,
            drag_state: DragState::None,
            sidebar: SidebarState {
                left_visible: true,
                left_width: 200.0,
                right_visible: true,
                right_width: 200.0,
            },
            active_tab: 0,
            sidebar_tree,
            tab_names: vec!["Main".to_string()],
            mouse_pos: (0.0, 0.0),
            modifiers: winit::keyboard::ModifiersState::default(),
            last_focused: None,
            last_visited_ws_idx: None,
            last_visited_pane_per_ws: vec![None; ws_count],
            mouse_enabled: self.app_config.config.general.mouse,
            last_render_time: None,
        })
    }

    fn render(&mut self) {
        let state = self.state.as_mut().unwrap();
        if !state.needs_redraw {
            return;
        }
        state.needs_redraw = false;
        state.last_render_time = Some(std::time::Instant::now());

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
                40.0
            },
            right_sidebar_width: if state.sidebar.right_visible {
                state.sidebar.right_width
            } else {
                40.0
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

        // ── TAB BAR ──
        let tb = &chrome;
        let side_bg = if theme.name == "Catppuccin Mocha" {
            [0.067, 0.067, 0.106, 1.0]
        } else {
            [0.953, 0.957, 0.973, 1.0]
        };
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
        let pane_count = state.session.active_workspace()
            .map(|ws| ws.scrolling.columns.iter().map(|c| c.panes.len()).sum::<usize>())
            .unwrap_or(0);
        let focus_title = state.session.active_workspace()
            .and_then(|ws| ws.scrolling.active_pane())
            .map(|p| p.title.as_str())
            .unwrap_or("—");
        let (mode_str, rename_hint) = match &state.input_mode {
            InputMode::Normal => ("NORMAL", String::new()),
            InputMode::Prefix => ("PREFIX", String::new()),
            InputMode::PaneSelect { .. } => ("SELECT", String::new()),
            InputMode::PaneSwap { .. } => ("SWAP", String::new()),
            InputMode::SidebarNav => ("SIDEBAR", String::new()),
            InputMode::Rename { target, buffer } => {
                ("RENAME", format!(": {}_", buffer))
            }
        };
        let status = format!("{} panes | {} | {}{}", pane_count, focus_title, mode_str, rename_hint);
        let status_text_y = sb_y + (tb.status_bar_height - chrome_text) / 2.0;
        state.text_renderer.queue_text(
            &status, 8.0, status_text_y, chrome_text, theme.foreground.to_f32x4(),
        );

        // ── Flush tab bar + status bar ──
        state.primitive_renderer.render(&state.device, &view, &mut encoder);
        state.text_renderer.render(&state.device, &state.queue, &view, &mut encoder);

        // ── PANE CONTENT AREA ──
        let theme_border = theme.border.to_f32x4();
        let border_width = theme.border_width;
        let accent_color = theme.accent.to_f32x4();

        // Get pane positions from NIRI layout engine
        let pane_positions = state.session.active_workspace()
            .map(|ws| ws.scrolling.panes_with_positions())
            .unwrap_or_default();

        // Get workspace geometry for overview (normal mode = full size)
        let ws_geometries = state.session.workspace_geometries();
        let ws_offset = ws_geometries.first()
            .map(|(_, rect)| (rect.loc.x as f32, rect.loc.y as f32))
            .unwrap_or((0.0, 0.0));

        // ── PANES ──
        for (pane_id, rect) in &pane_positions {
            let px = pane_area.x + ws_offset.0 + rect.loc.x as f32;
            let py = pane_area.y + ws_offset.1 + rect.loc.y as f32;
            let pw = rect.size.w as f32;
            let ph = rect.size.h as f32;
            let is_active = state.focused_pane == Some(pane_id.0);
            let bcolor = if is_active { accent_color } else {
                [theme_border[0], theme_border[1], theme_border[2], 0.5]
            };

            if let Some(backend) = state.backends.get(&pane_id.0) {
                let data = backend.render_data();
                render_backend_data(
                    &data, px, py, pw, ph,
                    &mut state.text_renderer, &mut state.primitive_renderer, theme,
                );
            } else {
                state.primitive_renderer.draw_rect(px, py, pw, ph, [0.118, 0.118, 0.180, 1.0]);
            }

            // Draw pane name as large centered label so you can tell panes apart
            let pane_name = state.session.active_workspace()
                .and_then(|ws| ws.find_pane(*pane_id))
                .map(|p| p.title.as_str())
                .unwrap_or("?");
            let name_size = (pw.min(ph) * 0.25).max(24.0).min(72.0);
            let name_color = if is_active {
                [1.0, 1.0, 1.0, 0.9]
            } else {
                [1.0, 1.0, 1.0, 0.4]
            };
            // Center the text
            let name_w = name_size * pane_name.len() as f32 * 0.6;
            let name_x = px + (pw - name_w) / 2.0;
            let name_y = py + (ph - name_size) / 2.0;
            state.text_renderer.queue_text(pane_name, name_x, name_y, name_size, name_color);

            state.primitive_renderer.draw_border(px, py, pw, ph, bcolor, border_width);
        }
        state.primitive_renderer.render(&state.device, &view, &mut encoder);
        state.text_renderer.render(&state.device, &state.queue, &view, &mut encoder);

        // ── SIDEBARS (drawn ON TOP of panes so they cover any overflow) ──
        let sidebar_top = chrome.tab_bar_height;
        let sidebar_bottom = h - chrome.status_bar_height;
        let sidebar_h = sidebar_bottom - sidebar_top;

        // Left sidebar
        state.primitive_renderer.draw_rect(0.0, sidebar_top, chrome.left_sidebar_width, sidebar_h, side_bg);
        state.primitive_renderer.draw_border(
            chrome.left_sidebar_width - 1.0, sidebar_top, 1.0, sidebar_h,
            theme.border.to_f32x4(), 1.0,
        );
        if chrome.left_sidebar_width >= 80.0 {
            sidebar::render_sidebar_expanded(
                &state.sidebar_tree,
                0.0, sidebar_top, chrome.left_sidebar_width, sidebar_h,
                matches!(state.input_mode, InputMode::SidebarNav),
                theme.accent.to_f32x4(),
                theme.foreground.to_f32x4(),
                [side_bg[0] * 2.0, side_bg[1] * 2.0, side_bg[2] * 2.0, 0.6],  // cursor highlight
                [theme.accent.to_f32x4()[0], theme.accent.to_f32x4()[1], theme.accent.to_f32x4()[2], 0.5],
                &mut state.text_renderer,
                &mut state.primitive_renderer,
            );
        } else {
            sidebar::render_sidebar_collapsed(
                &state.sidebar_tree,
                0.0, sidebar_top, chrome.left_sidebar_width, sidebar_h,
                theme.accent.to_f32x4(),
                theme.foreground.to_f32x4(),
                [theme.accent.to_f32x4()[0], theme.accent.to_f32x4()[1], theme.accent.to_f32x4()[2], 0.5],
                &mut state.text_renderer,
                &mut state.primitive_renderer,
            );
        }

        // Right sidebar
        let rsx = w - chrome.right_sidebar_width;
        state.primitive_renderer.draw_rect(rsx, sidebar_top, chrome.right_sidebar_width, sidebar_h, side_bg);
        state.primitive_renderer.draw_border(
            rsx, sidebar_top, 1.0, sidebar_h,
            theme.border.to_f32x4(), 1.0,
        );
        if chrome.right_sidebar_width >= 80.0 {
            state.text_renderer.queue_text(
                "Details", rsx + 8.0, sidebar_top + 8.0, chrome_text, theme.foreground.to_f32x4(),
            );
        }

        // ── FLOATING PANES ──
        if let Some(ws) = state.session.active_workspace() {
            for float in &ws.floating_panes {
                let fx = float.position.x as f32 + pane_area.x;
                let fy = float.position.y as f32 + pane_area.y;
                let fw = float.size.w as f32;
                let fh = float.size.h as f32;
                let is_focused = state.focused_pane == Some(float.pane.id.0);
                let fborder = if is_focused { theme.float_focus.to_f32x4() } else { theme.float_accent.to_f32x4() };
                state.primitive_renderer.draw_rect(fx, fy, fw, fh, theme.float_background.to_f32x4());
                state.primitive_renderer.draw_border(fx, fy, fw, fh, fborder, border_width * 2.0);
                if let Some(backend) = state.backends.get(&float.pane.id.0) {
                    let data = backend.render_data();
                    render_backend_data(&data, fx, fy, fw, fh, &mut state.text_renderer, &mut state.primitive_renderer, theme);
                }
                // Draw floating pane name centered
                let float_name = &float.pane.title;
                let f_name_size = (fw.min(fh) * 0.25).max(24.0).min(72.0);
                let f_name_color = if is_focused { [1.0, 1.0, 1.0, 0.9] } else { [1.0, 1.0, 1.0, 0.4] };
                let f_name_w = f_name_size * float_name.len() as f32 * 0.6;
                let f_name_x = fx + (fw - f_name_w) / 2.0;
                let f_name_y = fy + (fh - f_name_size) / 2.0;
                state.text_renderer.queue_text(float_name, f_name_x, f_name_y, f_name_size, f_name_color);
            }
        }

        // ── Pane select / swap letter overlay ──
        if let Some(candidates) = state.input_mode.candidates() {
            let letter_size = 48.0f32;
            let label_color = [1.0, 0.9, 0.3, 0.9];
            for (ch, target_id) in candidates {
                let mut found = false;
                // Check scrolling panes
                for (pane_id, rect) in &pane_positions {
                    if pane_id.0 == *target_id {
                        let px = pane_area.x + ws_offset.0 + rect.loc.x as f32;
                        let py = pane_area.y + ws_offset.1 + rect.loc.y as f32;
                        let pw = rect.size.w as f32;
                        let ph = rect.size.h as f32;
                        let lx = px + (pw - letter_size * 0.6) / 2.0;
                        let ly = py + (ph - letter_size) / 2.0;
                        let label = ch.to_string();
                        state.text_renderer.queue_text(&label, lx, ly, letter_size, label_color);
                        found = true;
                        break;
                    }
                }
                if !found {
                    // Check floating panes
                    if let Some(ws) = state.session.active_workspace() {
                        for float in &ws.floating_panes {
                            if float.pane.id.0 == *target_id {
                                let fx = float.position.x as f32 + pane_area.x;
                                let fy = float.position.y as f32 + pane_area.y;
                                let fw = float.size.w as f32;
                                let fh = float.size.h as f32;
                                let lx = fx + (fw - letter_size * 0.6) / 2.0;
                                let ly = fy + (fh - letter_size) / 2.0;
                                let label = ch.to_string();
                                state.text_renderer.queue_text(&label, lx, ly, letter_size, label_color);
                                break;
                            }
                        }
                    }
                }
            }
        }

        state.primitive_renderer.render(&state.device, &view, &mut encoder);
        state.text_renderer.render(&state.device, &state.queue, &view, &mut encoder);

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
                    // Update NIRI session viewport to content area size
                    update_session_viewport(state);
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

                #[cfg(debug_assertions)]
                eprintln!("input: mode={:?} log_key={:?} text='{}' ctrl={} shift={} phys={:?}",
                    state.input_mode, log_key, key_text, is_ctrl, is_shift, phys);

                // Detect Ctrl+B by multiple methods:
                let is_prefix = key_text == "\u{2}"                         // macOS control char
                    || matches!(log_key, winit::keyboard::Key::Character(c) if c == "\u{2}")
                    || (is_ctrl && phys == winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyB));

                // Handle Rename mode separately (needs mutable buffer access)
                if let InputMode::Rename { target, buffer } = &mut state.input_mode {
                    let is_escape = matches!(event.logical_key, winit::keyboard::Key::Named(NamedKey::Escape));
                    let is_enter = matches!(event.logical_key, winit::keyboard::Key::Named(NamedKey::Enter));
                    let is_backspace = matches!(event.logical_key, winit::keyboard::Key::Named(NamedKey::Backspace));

                    if is_escape {
                        state.input_mode = InputMode::Normal;
                    } else if is_enter {
                        // Commit rename
                        let new_name = buffer.trim().to_string();
                        match target {
                            RenameTarget::Workspace(ws_idx) => {
                                if let Some(ws) = state.session.workspaces.get_mut(*ws_idx) {
                                    ws.name = if new_name.is_empty() { None } else { Some(new_name) };
                                }
                            }
                            RenameTarget::Pane(pane_id) => {
                                if let Some(ws) = state.session.active_workspace_mut() {
                                    if let Some(pane) = ws.find_pane_mut(heca_core::layout::PaneId(*pane_id)) {
                                        pane.title = if new_name.is_empty() { format!("pane{}", pane_id) } else { new_name };
                                    }
                                }
                            }
                        }
                        sync_focus(state);
                        state.input_mode = InputMode::Normal;
                    } else if is_backspace {
                        buffer.pop();
                    } else if key_text.len() == 1 && !is_ctrl {
                        buffer.push_str(&key_text);
                    }
                    state.needs_redraw = true;
                    return;
                }

                // Detect Ctrl+C, Ctrl+D etc. for future pane forwarding
                // Detect arrow keys via physical_key
                match &state.input_mode {
                    InputMode::Normal => {
                        // Only prefix key activates prefix mode in Normal
                        if is_prefix {
                            state.input_mode = InputMode::Prefix;
                            state.needs_redraw = true;
                            return;
                        }

                        // Forward key to focused pane's backend (terminal input)
                        if let Some(pane_id) = state.focused_pane {
                            if let Some(backend) = state.backends.get_mut(&pane_id) {
                                let input_bytes = winit_key_to_terminal_input(
                                    &event.logical_key,
                                    &key_text,
                                    is_ctrl,
                                );
                                if !input_bytes.is_empty() {
                                    backend.process_input(&input_bytes);
                                    return;
                                }
                            }
                        }
                    }
                    InputMode::Prefix => {
                        if is_prefix {
                            // Double-prefix: forward literal Ctrl+B (0x02) to the terminal
                            // so nested tmux/screen work correctly.
                            state.input_mode = InputMode::Normal;
                            if let Some(pane_id) = state.focused_pane {
                                if let Some(backend) = state.backends.get_mut(&pane_id) {
                                    backend.process_input(&[0x02]);
                                }
                            }
                            return;
                        }

                        // Ignore bare modifier keys (Shift, etc.) in prefix mode.
                        // The user may hold Shift while pressing the action key; we should
                        // wait for the actual character key, not exit on Shift alone.
                        let is_modifier_only = key_text.is_empty()
                            && matches!(event.logical_key, winit::keyboard::Key::Named(
                                winit::keyboard::NamedKey::Shift
                                | winit::keyboard::NamedKey::Control
                                | winit::keyboard::NamedKey::Alt
                                | winit::keyboard::NamedKey::Super
                                | winit::keyboard::NamedKey::Hyper
                                | winit::keyboard::NamedKey::Meta
                            ));
                        if is_modifier_only {
                            return;
                        }

                        // In prefix mode, pass the REAL modifier state. The user may intentionally
                        // press Ctrl+another key after the prefix (e.g. Ctrl+h for swap_left).
                        // The prefix key itself (Ctrl+B) is already handled above by is_prefix.
                        let action = self.bindings.resolve(&key_text, is_ctrl, false, is_shift, &event.logical_key, &event.physical_key);
                        #[cfg(debug_assertions)]
                        eprintln!("prefix: key_text='{}' shift={} phys={:?} action={:?}", key_text, is_shift, event.physical_key, action);

                        // Only reset to Normal if we found an action or the key is printable.
                        // If no action matched and key_text is empty, stay in prefix (e.g. dead keys).
                        if let Some(act) = action {
                            state.input_mode = InputMode::Normal;
                            execute_action(act, state.focused_pane, state);
                        } else if !key_text.is_empty() {
                            // Printable key that didn't match any binding — exit prefix.
                            state.input_mode = InputMode::Normal;
                        }
                        // else: empty key_text, no action — stay in prefix mode.
                    }
                    InputMode::PaneSelect { candidates } => {
                        let typed = key_text.chars().next()
                            .or_else(|| {
                                match event.physical_key {
                                    winit::keyboard::PhysicalKey::Code(c) => {
                                        let s = format!("{:?}", c);
                                        s.strip_prefix("Key").and_then(|n| n.chars().next())
                                    }
                                    _ => None,
                                }
                            })
                            .map(|c| c.to_ascii_lowercase());
                        #[cfg(debug_assertions)]
                        eprintln!("pane_select: typed={:?} candidates={:?}", typed, candidates);
                        if let Some(ch) = typed {
                            if let Some((_, target_id)) = candidates.iter().find(|(c, _)| *c == ch) {
                                focus_pane_by_id(state, *target_id);
                            }
                        }
                        state.input_mode = InputMode::Normal;
                    }
                    InputMode::PaneSwap { candidates } => {
                        let typed = key_text.chars().next()
                            .or_else(|| {
                                match event.physical_key {
                                    winit::keyboard::PhysicalKey::Code(c) => {
                                        let s = format!("{:?}", c);
                                        s.strip_prefix("Key").and_then(|n| n.chars().next())
                                    }
                                    _ => None,
                                }
                            })
                            .map(|c| c.to_ascii_lowercase());
                        #[cfg(debug_assertions)]
                        eprintln!("pane_swap: typed={:?} candidates={:?}", typed, candidates);
                        if let Some(ch) = typed {
                            if let Some((_, target_id)) = candidates.iter().find(|(c, _)| *c == ch) {
                                if let Some(current_id) = state.focused_pane {
                                    if current_id != *target_id {
                                        swap_panes(state, current_id, *target_id);
                                    }
                                }
                            }
                        }
                        state.input_mode = InputMode::Normal;
                    }
                    InputMode::SidebarNav => {
                        // Sidebar mode uses its OWN keybinding set (mode_bindings["sidebar"])
                        // so j/k/h/l etc. don't conflict with normal mode bindings.
                        let is_escape = matches!(event.logical_key, winit::keyboard::Key::Named(NamedKey::Escape));

                        if is_escape {
                            state.input_mode = InputMode::Normal;
                            state.needs_redraw = true;
                        } else {
                            let action = self.bindings.resolve_mode(
                                "sidebar",
                                &key_text, is_ctrl, false, is_shift,
                                &event.logical_key, &event.physical_key,
                            );
                            #[cfg(debug_assertions)]
                            eprintln!("sidebar: key_text='{}' phys={:?} action={:?}", key_text, event.physical_key, action);
                            if let Some(act) = action {
                                execute_action(act, state.focused_pane, state);
                            }
                        }
                    }
                    InputMode::Rename { .. } => {
                        // Handled by early return before this match
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

                if !state.mouse_enabled { return; }
            }
            WindowEvent::MouseInput { state: button_state, button, .. } => {
                if !state.mouse_enabled { return; }
                state.needs_redraw = true;
                let mouse_pos = state.mouse_pos;
                let phys = state.window.inner_size();
                let win_w = phys.width as f32 / state.scale_factor as f32;
                let win_h = phys.height as f32 / state.scale_factor as f32;
                let chrome = ChromeConfig {
                    tab_bar_height: 32.0,
                    status_bar_height: 24.0,
                    left_sidebar_width: if state.sidebar.left_visible { state.sidebar.left_width } else { 40.0 },
                    right_sidebar_width: if state.sidebar.right_visible { state.sidebar.right_width } else { 40.0 },
                };
                let pane_area = chrome.content_rect(win_w, win_h);

                if button == MouseButton::Left && button_state == ElementState::Pressed {
                    // Hit test against NIRI layout pane positions
                    let pane_positions = state.session.active_workspace()
                        .map(|ws| ws.scrolling.panes_with_positions())
                        .unwrap_or_default();
                    let ws_geometries = state.session.workspace_geometries();
                    let ws_offset = ws_geometries.first()
                        .map(|(_, rect)| (rect.loc.x as f32, rect.loc.y as f32))
                        .unwrap_or((0.0, 0.0));

                    for (pane_id, rect) in &pane_positions {
                        let px = pane_area.x + ws_offset.0 + rect.loc.x as f32;
                        let py = pane_area.y + ws_offset.1 + rect.loc.y as f32;
                        let pw = rect.size.w as f32;
                        let ph = rect.size.h as f32;
                        if mouse_pos.0 >= px && mouse_pos.0 <= px + pw
                            && mouse_pos.1 >= py && mouse_pos.1 <= py + ph
                        {
                            state.focused_pane = Some(pane_id.0);
                            sync_focus(state);
                            break;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(ref mut state) = self.state {
            // Advance session animations.
            state.session.advance_animations();

            // Poll backends (fake backends return false)
            let mut backend_has_data = false;
            for backend in state.backends.values_mut() {
                if backend.update() {
                    backend_has_data = true;
                }
            }

            let needs_frame = state.needs_redraw || backend_has_data || state.session.are_animations_ongoing();
            if needs_frame {
                state.window.request_redraw();
            }

            // Use WaitUntil during animations (60fps cap), Wait when idle (0% CPU).
            if state.session.are_animations_ongoing() {
                event_loop.set_control_flow(ControlFlow::WaitUntil(
                    Instant::now() + Duration::from_millis(16)
                ));
            } else {
                event_loop.set_control_flow(ControlFlow::Wait);
            }
        }
    }
}

/// Focus a specific pane by its ID, updating both `focused_pane` and the session's active state.
/// Does NOT move the layout — compensates view_offset so pane positions stay visually fixed.
fn focus_pane_by_id(state: &mut AppState, pane_id: u64) {
    // Track visited
    if let Some(old_pane_id) = state.focused_pane {
        let ws_idx = state.session.active_workspace_idx;
        while state.last_visited_pane_per_ws.len() <= ws_idx {
            state.last_visited_pane_per_ws.push(None);
        }
        state.last_visited_pane_per_ws[ws_idx] = Some(old_pane_id);
    }
    state.focused_pane = Some(pane_id);
    if let Some(ws) = state.session.active_workspace_mut() {
        // Find location first (immutable scan), then mutate.
        let mut found = None;
        for (ci, col) in ws.scrolling.columns.iter().enumerate() {
            for (pi, pane) in col.panes.iter().enumerate() {
                if pane.id.0 == pane_id {
                    found = Some((ci, pi));
                    break;
                }
            }
            if found.is_some() { break; }
        }

        if let Some((ci, pi)) = found {
            ws.floating_is_active = false;
            // Compensate view_offset so the visual layout doesn't jump.
            // view_pos = column_x(active_column_idx) + view_offset.
            // We want view_pos to stay the same after changing active_column_idx.
            let old_col_x = ws.scrolling.column_x(ws.scrolling.active_column_idx);
            let new_col_x = ws.scrolling.column_x(ci);
            let delta = old_col_x - new_col_x;
            ws.scrolling.view_offset = heca_core::layout::view_offset::ViewOffset::Static(
                ws.scrolling.view_offset.current() + delta
            );
            ws.scrolling.active_column_idx = ci;
            if let Some(col) = ws.scrolling.columns.get_mut(ci) {
                col.active_pane_idx = pi;
            }
            return;
        }

        // Check floating panes
        for float in &mut ws.floating_panes {
            if float.pane.id.0 == pane_id {
                ws.floating_is_active = true;
                float.is_active = true;
                return;
            }
        }
    }
}

/// Sync `focused_pane` from the session's active pane (scrolling or floating).
/// Also tracks last-visited workspace and rebuilds the sidebar tree.
fn sync_focus(state: &mut AppState) {
    // Track last visited workspace before updating
    if let Some(old_ws_idx) = state.last_visited_ws_idx {
        if old_ws_idx != state.session.active_workspace_idx {
            // Record which pane was active in the departing workspace
            if let Some(ws) = state.session.workspaces.get(old_ws_idx) {
                if let Some(pane) = ws.active_pane() {
                    while state.last_visited_pane_per_ws.len() <= old_ws_idx {
                        state.last_visited_pane_per_ws.push(None);
                    }
                    state.last_visited_pane_per_ws[old_ws_idx] = Some(pane.id.0);
                }
            }
        }
    }
    state.last_visited_ws_idx = Some(state.session.active_workspace_idx);

    state.focused_pane = state.session.active_workspace()
        .and_then(|ws| ws.active_pane())
        .map(|p| p.id.0);

    // Rebuild sidebar tree
    state.sidebar_tree.rebuild(
        &state.session,
        state.last_visited_ws_idx,
        state.focused_pane,
    );
}

/// Update session viewport to match current chrome/content area size.
fn update_session_viewport(state: &mut AppState) {
    let phys = state.window.inner_size();
    let win_w = phys.width as f32 / state.scale_factor as f32;
    let win_h = phys.height as f32 / state.scale_factor as f32;
    let chrome = ChromeConfig {
        tab_bar_height: 32.0,
        status_bar_height: 24.0,
        left_sidebar_width: if state.sidebar.left_visible { state.sidebar.left_width } else { 40.0 },
        right_sidebar_width: if state.sidebar.right_visible { state.sidebar.right_width } else { 40.0 },
    };
    let pane_area = chrome.content_rect(win_w, win_h);
    let new_size = heca_core::layout::types::Size::new(
        pane_area.w as f64,
        pane_area.h as f64,
    );
    state.session.update_viewport(new_size);
}

fn execute_action(action: WmAction, _current: Option<u64>, state: &mut AppState) {
    match action {
        WmAction::FocusLeft => {
            state.session.focus_left();
            sync_focus(state);
            state.needs_redraw = true;
        }
        WmAction::FocusRight => {
            state.session.focus_right();
            sync_focus(state);
            state.needs_redraw = true;
        }
        WmAction::FocusUp => {
            // Stay within workspace — don't wrap to previous workspace
            if let Some(ws) = state.session.active_workspace_mut() {
                ws.focus_up();
            }
            sync_focus(state);
            state.needs_redraw = true;
        }
        WmAction::FocusDown => {
            // Stay within workspace — don't wrap to next workspace
            if let Some(ws) = state.session.active_workspace_mut() {
                ws.focus_down();
            }
            sync_focus(state);
            state.needs_redraw = true;
        }
        WmAction::SplitHorizontal => {
            #[cfg(debug_assertions)]
            eprintln!("execute_action: SplitHorizontal — creating new column");
            // New column to the right
            let next_id = state.session.next_id();
            let pane = LayoutPane::new(PaneId(next_id), &pane_name(next_id));
            let backend_id = next_id;
            state.session.add_pane(pane, None, true);
            state.backends.insert(backend_id, Box::new(FakeBackend::new(80, 24)));
            sync_focus(state);
            state.needs_redraw = true;
        }
        WmAction::SplitVertical => {
            // New pane in current column
            let next_id = state.session.next_id();
            let pane = LayoutPane::new(PaneId(next_id), &pane_name(next_id));
            let backend_id = next_id;
            let col_idx = state.session.active_workspace()
                .map(|ws| ws.scrolling.active_column_idx)
                .unwrap_or(0);
            state.session.active_workspace_mut()
                .map(|ws| ws.scrolling.add_pane_to_column(col_idx, None, pane, true));
            state.backends.insert(backend_id, Box::new(FakeBackend::new(80, 24)));
            sync_focus(state);
            state.needs_redraw = true;
        }
        WmAction::ClosePane => {
            // Count total panes across all columns
            let total_panes = state.session.active_workspace()
                .map(|ws| ws.scrolling.columns.iter().map(|c| c.panes.len()).sum::<usize>())
                .unwrap_or(0);
            if total_panes <= 1 {
                // Don't close the last pane — workspace would become empty
                state.needs_redraw = true;
                return;
            }
            if let Some(ws) = state.session.active_workspace_mut() {
                let col_idx = ws.scrolling.active_column_idx;
                if let Some(col) = ws.scrolling.active_column() {
                    let pane_idx = col.active_pane_idx;
                    if let Some(removed) = ws.scrolling.remove_pane(col_idx, pane_idx) {
                        state.backends.remove(&removed.id.0);
                    }
                }
            }
            sync_focus(state);
            state.needs_redraw = true;
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
        WmAction::SidebarLeft => {
            state.sidebar.left_visible = !state.sidebar.left_visible;
            update_session_viewport(state);
            state.needs_redraw = true;
        }
        WmAction::SidebarRight => {
            state.sidebar.right_visible = !state.sidebar.right_visible;
            update_session_viewport(state);
            state.needs_redraw = true;
        }
        WmAction::NextPane => {
            state.session.focus_right();
            sync_focus(state);
            state.needs_redraw = true;
        }
        WmAction::PrevPane => {
            state.session.focus_left();
            sync_focus(state);
            state.needs_redraw = true;
        }
        WmAction::ResizeIncrease => {
            if let Some(ws) = state.session.active_workspace_mut() {
                ws.scrolling.resize_active_column(0.05);
            }
            state.needs_redraw = true;
        }
        WmAction::ResizeDecrease => {
            if let Some(ws) = state.session.active_workspace_mut() {
                ws.scrolling.resize_active_column(-0.05);
            }
            state.needs_redraw = true;
        }
        WmAction::PaneHeightIncrease => {
            if let Some(ws) = state.session.active_workspace_mut() {
                let col_idx = ws.scrolling.active_column_idx;
                if let Some(col) = ws.scrolling.columns.get_mut(col_idx) {
                    let h = ws.scrolling.working_area.size.h;
                    let gaps = ws.scrolling.options.gaps;
                    col.resize_active_pane_height(40.0, h, gaps);
                }
            }
            state.needs_redraw = true;
        }
        WmAction::PaneHeightDecrease => {
            if let Some(ws) = state.session.active_workspace_mut() {
                let col_idx = ws.scrolling.active_column_idx;
                if let Some(col) = ws.scrolling.columns.get_mut(col_idx) {
                    let h = ws.scrolling.working_area.size.h;
                    let gaps = ws.scrolling.options.gaps;
                    col.resize_active_pane_height(-40.0, h, gaps);
                }
            }
            state.needs_redraw = true;
        }
        WmAction::MovePaneLeft => {
            if let Some(ws) = state.session.active_workspace_mut() {
                ws.scrolling.move_active_pane_left();
            }
            sync_focus(state);
            state.needs_redraw = true;
        }
        WmAction::MovePaneRight => {
            if let Some(ws) = state.session.active_workspace_mut() {
                ws.scrolling.move_active_pane_right();
            }
            sync_focus(state);
            state.needs_redraw = true;
        }
        WmAction::PaneSelect => {
            let mut candidates: Vec<(char, u64)> = Vec::new();
            if let Some(ws) = state.session.active_workspace() {
                for col in &ws.scrolling.columns {
                    for pane in &col.panes {
                        let ch = (b'a' + candidates.len() as u8) as char;
                        candidates.push((ch, pane.id.0));
                    }
                }
                for float in &ws.floating_panes {
                    let ch = (b'a' + candidates.len() as u8) as char;
                    candidates.push((ch, float.pane.id.0));
                }
            }
            if !candidates.is_empty() {
                state.input_mode = InputMode::PaneSelect { candidates };
                state.needs_redraw = true;
            }
        }
        WmAction::SwapSelect => {
            let mut candidates: Vec<(char, u64)> = Vec::new();
            if let Some(ws) = state.session.active_workspace() {
                for col in &ws.scrolling.columns {
                    for pane in &col.panes {
                        let ch = (b'a' + candidates.len() as u8) as char;
                        candidates.push((ch, pane.id.0));
                    }
                }
                for float in &ws.floating_panes {
                    let ch = (b'a' + candidates.len() as u8) as char;
                    candidates.push((ch, float.pane.id.0));
                }
            }
            if !candidates.is_empty() {
                state.input_mode = InputMode::PaneSwap { candidates };
                state.needs_redraw = true;
            }
        }
        WmAction::SwapLeft => {
            if let Some(ws) = state.session.active_workspace_mut() {
                ws.scrolling.move_column_left();
            }
            state.needs_redraw = true;
        }
        WmAction::SwapRight => {
            if let Some(ws) = state.session.active_workspace_mut() {
                ws.scrolling.move_column_right();
            }
            state.needs_redraw = true;
        }
        WmAction::SwapUp | WmAction::SwapDown => {
            if let Some(ws) = state.session.active_workspace_mut() {
                let col_idx = ws.scrolling.active_column_idx;
                if let Some(col) = ws.scrolling.active_column() {
                    let pane_idx = col.active_pane_idx;
                    let swap_with = if matches!(action, WmAction::SwapUp) {
                        pane_idx.saturating_sub(1)
                    } else {
                        (pane_idx + 1).min(col.panes.len().saturating_sub(1))
                    };
                    if swap_with != pane_idx {
                        if let Some(col) = ws.scrolling.columns.get_mut(col_idx) {
                            // Compute Y offsets BEFORE swap for animation.
                            let h_above = col.pane_sizes.get(pane_idx.min(swap_with))
                                .map(|s| s.h).unwrap_or(0.0);
                            let h_below = col.pane_sizes.get(pane_idx.max(swap_with))
                                .map(|s| s.h).unwrap_or(0.0);
                            let gap = ws.scrolling.options.gaps;

                            // Animate the two panes swapping positions.
                            // Pane moving UP starts from below and slides up.
                            // Pane moving DOWN starts from above and slides down.
                            let up_offset = h_above + gap;
                            let down_offset = -(h_below + gap);

                            // Apply animation BEFORE the swap (so we animate the right panes).
                            if swap_with < pane_idx {
                                // SwapUp: pane at idx (lower) moves up, pane at idx-1 (upper) moves down.
                                col.panes[pane_idx].animate_move_y_from(up_offset, AnimationConfig::default());
                                col.panes[swap_with].animate_move_y_from(down_offset, AnimationConfig::default());
                            } else {
                                // SwapDown: pane at idx (upper) moves down, pane at idx+1 (lower) moves up.
                                col.panes[pane_idx].animate_move_y_from(down_offset, AnimationConfig::default());
                                col.panes[swap_with].animate_move_y_from(up_offset, AnimationConfig::default());
                            }

                            col.panes.swap(pane_idx, swap_with);
                            col.active_pane_idx = swap_with;
                            col.compute_pane_sizes(ws.scrolling.working_area.size.h, ws.scrolling.options.gaps);
                        }
                    }
                }
            }
            sync_focus(state);
            state.needs_redraw = true;
        }
        WmAction::Float => {
            if let Some(pane_id) = state.focused_pane {
                if let Some(ws) = state.session.active_workspace_mut() {
                    let wa = ws.scrolling.working_area;
                    let is_floating = ws.floating_panes.iter().any(|f| f.pane.id.0 == pane_id);

                    if is_floating {
                        // Tiling ← Floating: remove from floating, restore to original column
                        if let Some(idx) = ws.floating_panes.iter().position(|f| f.pane.id.0 == pane_id) {
                            let float = ws.floating_panes.remove(idx);
                            let orig_col = float.original_column_idx;
                            let orig_pane = float.original_pane_idx;
                            if let Some(col_idx) = orig_col {
                                if col_idx < ws.scrolling.columns.len() {
                                    let target_idx = orig_pane.unwrap_or(0).min(ws.scrolling.columns[col_idx].panes.len());
                                    ws.scrolling.columns[col_idx].panes.insert(target_idx, float.pane);
                                    ws.scrolling.columns[col_idx].active_pane_idx = target_idx;
                                    ws.scrolling.active_column_idx = col_idx;
                                    ws.scrolling.update_all_column_widths();
                                } else {
                                    ws.scrolling.add_column(None, Column::new(
                                        ColumnId(pane_id), float.pane, ColumnWidth::Proportion(0.5),
                                    ), true);
                                }
                            } else {
                                ws.scrolling.add_column(None, Column::new(
                                    ColumnId(pane_id), float.pane, ColumnWidth::Proportion(0.5),
                                ), true);
                            }
                            ws.floating_is_active = false;
                        }
                    } else {
                        // Tiling → Floating: centered, 75% of working area
                        let mut found = None;
                        for (ci, col) in ws.scrolling.columns.iter().enumerate() {
                            for (pi, pane) in col.panes.iter().enumerate() {
                                if pane.id.0 == pane_id { found = Some((ci, pi)); break; }
                            }
                            if found.is_some() { break; }
                        }
                        if let Some((col_idx, pane_idx)) = found {
                            if let Some(removed) = ws.scrolling.remove_pane(col_idx, pane_idx) {
                                let fw = wa.size.w * 0.75;
                                let fh = wa.size.h * 0.75;
                                let fx = wa.loc.x + (wa.size.w - fw) / 2.0;
                                let fy = wa.loc.y + (wa.size.h - fh) / 2.0;
                                ws.floating_panes.push(heca_core::layout::workspace::FloatingPane {
                                    pane: removed,
                                    position: heca_core::layout::types::Point::new(fx, fy),
                                    size: heca_core::layout::types::Size::new(fw, fh),
                                    is_active: true,
                                    original_column_idx: Some(col_idx),
                                    original_pane_idx: Some(pane_idx),
                                });
                                ws.floating_is_active = true;
                            }
                        }
                    }
                }
            }
            sync_focus(state);
            state.needs_redraw = true;
        }
        WmAction::SidebarFocus => {
            // Enter sidebar navigation mode
            state.input_mode = InputMode::SidebarNav;
            // Auto-expand: rebuild tree if collapsed items exist
            state.sidebar_tree.rebuild(
                &state.session,
                state.last_visited_ws_idx,
                state.focused_pane,
            );
            state.needs_redraw = true;
        }
        WmAction::SidebarUp => {
            if matches!(state.input_mode, InputMode::SidebarNav) {
                state.sidebar_tree.cursor_up();
                state.needs_redraw = true;
            }
        }
        WmAction::SidebarDown => {
            if matches!(state.input_mode, InputMode::SidebarNav) {
                state.sidebar_tree.cursor_down();
                state.needs_redraw = true;
            }
        }
        WmAction::SidebarLeftNav => {
            if matches!(state.input_mode, InputMode::SidebarNav) {
                state.sidebar_tree.collapse();
                state.needs_redraw = true;
            }
        }
        WmAction::SidebarRightNav => {
            if matches!(state.input_mode, InputMode::SidebarNav) {
                let item = state.sidebar_tree.current_item().cloned();
                match &item {
                    Some(sidebar::SidebarItem::Pane { pane_id }) => {
                        // Focus the pane and exit sidebar nav
                        focus_pane_by_id(state, *pane_id);
                        state.input_mode = InputMode::Normal;
                    }
                    _ => {
                        state.sidebar_tree.expand();
                    }
                }
                state.needs_redraw = true;
            }
        }
        WmAction::SidebarExpandToggle => {
            if matches!(state.input_mode, InputMode::SidebarNav) {
                let item = state.sidebar_tree.current_item().cloned();
                match &item {
                    Some(sidebar::SidebarItem::Pane { pane_id }) => {
                        // Enter/expand on a pane focuses it
                        focus_pane_by_id(state, *pane_id);
                        state.input_mode = InputMode::Normal;
                    }
                    _ => {
                        state.sidebar_tree.toggle_expand();
                    }
                }
                state.needs_redraw = true;
            }
        }
        WmAction::WorkspaceNext => {
            let next = (state.session.active_workspace_idx + 1)
                .min(state.session.workspaces.len().saturating_sub(1));
            if next != state.session.active_workspace_idx {
                state.session.switch_to_workspace(next);
                sync_focus(state);
                state.needs_redraw = true;
            }
        }
        WmAction::WorkspacePrev => {
            let prev = state.session.active_workspace_idx.saturating_sub(1);
            if prev != state.session.active_workspace_idx {
                state.session.switch_to_workspace(prev);
                sync_focus(state);
                state.needs_redraw = true;
            }
        }
        WmAction::CreateWorkspace => {
            // Create a new empty workspace and switch to it
            let working_area = state.session.active_workspace()
                .map(|ws| Rectangle::new(ws.scrolling.working_area.loc, ws.scrolling.working_area.size))
                .unwrap_or_else(|| {
                    Rectangle::new(
                        heca_core::layout::types::Point::default(),
                        state.session.viewport_size,
                    )
                });
            state.session.add_workspace(working_area);
            let new_idx = state.session.workspaces.len() - 1;
            state.session.switch_to_workspace(new_idx);
            // Ensure last_visited_pane_per_ws is sized correctly
            while state.last_visited_pane_per_ws.len() <= new_idx {
                state.last_visited_pane_per_ws.push(None);
            }
            sync_focus(state);
            state.needs_redraw = true;
        }
        WmAction::RenameWorkspace => {
            // Enter rename mode for current workspace
            let ws_idx = state.session.active_workspace_idx;
            let current_name = state.session.active_workspace()
                .and_then(|ws| ws.name.clone())
                .unwrap_or_default();
            state.input_mode = InputMode::Rename {
                target: RenameTarget::Workspace(ws_idx),
                buffer: current_name,
            };
            state.needs_redraw = true;
        }
        WmAction::RenamePane => {
            // Enter rename mode for focused pane
            if let Some(pane_id) = state.focused_pane {
                let current_title = state.session.active_workspace()
                    .and_then(|ws| ws.find_pane(heca_core::layout::PaneId(pane_id)))
                    .map(|p| p.title.clone())
                    .unwrap_or_default();
                state.input_mode = InputMode::Rename {
                    target: RenameTarget::Pane(pane_id),
                    buffer: current_title,
                };
                state.needs_redraw = true;
            }
        }
        WmAction::Scratchpad | WmAction::Hide => {}
        WmAction::ResizeLeft | WmAction::ResizeUp | WmAction::ResizeRight | WmAction::ResizeDown => {}
    }
}

/// Swap two panes by their IDs. If the focused pane is involved, focus follows it.
fn swap_panes(state: &mut AppState, a_id: u64, b_id: u64) {
    if let Some(ws) = state.session.active_workspace_mut() {
        let mut a_col: Option<usize> = None;
        let mut a_idx: Option<usize> = None;
        let mut b_col: Option<usize> = None;
        let mut b_idx: Option<usize> = None;

        for (ci, col) in ws.scrolling.columns.iter().enumerate() {
            for (pi, pane) in col.panes.iter().enumerate() {
                if pane.id.0 == a_id {
                    a_col = Some(ci);
                    a_idx = Some(pi);
                }
                if pane.id.0 == b_id {
                    b_col = Some(ci);
                    b_idx = Some(pi);
                }
            }
        }

        if let (Some(ac), Some(ai), Some(bc), Some(bi)) = (a_col, a_idx, b_col, b_idx) {
            let focused_id = state.focused_pane;
            let focused_is_a = focused_id == Some(a_id);
            let focused_is_b = focused_id == Some(b_id);

            if ac == bc {
                ws.scrolling.columns[ac].panes.swap(ai, bi);
                if focused_is_a {
                    ws.scrolling.columns[ac].active_pane_idx = bi;
                } else if focused_is_b {
                    ws.scrolling.columns[ac].active_pane_idx = ai;
                }
            } else {
                // Swap pane structs between columns.
                // Scope the split_at_mut borrow so we can mutate active indices afterwards.
                {
                    let (col_a, col_b) = if ac < bc {
                        let (left, right) = ws.scrolling.columns.split_at_mut(bc);
                        (&mut left[ac], &mut right[0])
                    } else {
                        let (left, right) = ws.scrolling.columns.split_at_mut(ac);
                        (&mut right[0], &mut left[bc])
                    };
                    std::mem::swap(&mut col_a.panes[ai], &mut col_b.panes[bi]);
                }

                // Follow the focused pane to its new column so active indices stay consistent.
                if focused_is_a {
                    ws.scrolling.active_column_idx = bc;
                    ws.scrolling.columns[bc].active_pane_idx = bi;
                } else if focused_is_b {
                    ws.scrolling.active_column_idx = ac;
                    ws.scrolling.columns[ac].active_pane_idx = ai;
                }
            }

            // Recompute pane sizes since swapped panes may have different preferred heights.
            let h = ws.scrolling.working_area.size.h;
            let gaps = ws.scrolling.options.gaps;
            ws.scrolling.columns[ac].compute_pane_sizes(h, gaps);
            if bc != ac {
                ws.scrolling.columns[bc].compute_pane_sizes(h, gaps);
            }
            ws.scrolling.update_all_column_widths();

            // If the focused pane moved, animate the view so it stays visible.
            // Otherwise leave the viewport untouched.
            if focused_is_a || focused_is_b {
                ws.scrolling.align_view_to_active_column();
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
