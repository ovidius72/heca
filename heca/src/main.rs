mod actions;
mod app_state;
mod chrome;
mod handlers;
mod input;
mod keymap;
mod rpc;
mod sidebar;

use sidebar::SidebarTree;

use app_state::{AppState, SidebarState, InputMode, RenameTarget};
use chrome::ChromeConfig;
use heca_config::theme::AppConfig;
use heca_core::backend::{BackendRenderData, PaneBackend, FakeBackend};
use heca_core::layout::{Session, Column, Pane as LayoutPane, ColumnId, PaneId, ColumnWidth};
use heca_renderer::primitive::PrimitiveRenderer;
use heca_renderer::text::TextRenderer;
use input::{WmAction, action_from_name};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::{WindowEvent, MouseButton, ElementState};
use winit::keyboard::{Key, NamedKey};

use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};

use winit::window::{Window, WindowId};

/// Render a backend's content into a pane rectangle.
#[allow(clippy::too_many_arguments)]
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
    if let BackendRenderData::Terminal {
            lines,
            cursor_col,
            cursor_row,
        } = data {
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
}

/// Extended alphabet for pane/column candidate labels (52 chars).
const CANDIDATE_ALPHABET: &[char] = &[
    'a','b','c','d','e','f','g','h','i','j','k','l','m',
    'n','o','p','q','r','s','t','u','v','w','x','y','z',
    'A','B','C','D','E','F','G','H','I','J','K','L','M',
    'N','O','P','Q','R','S','T','U','V','W','X','Y','Z',
];

/// Distinct pane names so you can visually identify what's moving.
const PANE_NAMES: &[&str] = &[
    "Red", "Green", "Blue", "Yellow", "Cyan", "Magenta",
    "Orange", "Purple", "Lime", "Pink", "Teal", "Coral",
];

pub(crate) fn pane_name(id: u64) -> String {
    PANE_NAMES.get((id as usize).saturating_sub(1) % PANE_NAMES.len())
        .unwrap_or(&"?")
        .to_string()
}

/// Check if a key event's combo matches a configured KeyCombo.
/// Case-insensitive for alphabetic keys; exact otherwise.
fn event_combo_matches(event: &keymap::KeyCombo, configured: &keymap::KeyCombo) -> bool {
    if event.ctrl != configured.ctrl
        || event.shift != configured.shift
        || event.alt != configured.alt
        || event.super_ != configured.super_
    {
        return false;
    }
    // Case-insensitive match for single alphabetic characters.
    if event.key.len() == 1 && configured.key.len() == 1 {
        let e = event.key.chars().next().unwrap();
        let c = configured.key.chars().next().unwrap();
        e.eq_ignore_ascii_case(&c)
    } else {
        event.key.eq_ignore_ascii_case(&configured.key)
    }
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
        if c.is_ascii_lowercase() {
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
    registry: actions::ActionRegistry,
    keymap: keymap::KeymapRegistry,
}

impl HecaApp {
    /// Parse and execute an RPC command string.
    /// Returns the parsed action on success, or an error string on failure.
    // Transitional: will be used by the RPC server / socket listener in Phase 5.
    #[allow(dead_code)]
    pub fn execute_rpc_command(&mut self, cmd: &str) -> Result<WmAction, String> {
        let state = self.state.as_mut().ok_or("app not initialized")?;
        let action = rpc::parse_rpc_command(cmd).map_err(|e| e.to_string())?;
        self.registry.execute(&action, state);
        Ok(action)
    }

    fn new() -> Self {
        let app_config = AppConfig::load();
        let registry = build_registry();
        let mut keymap = keymap::KeymapRegistry::new();

        // ── Load bindings from [keys] flat map ──
        for (action_name, value) in &app_config.config.keys.bindings {
            let Some(action) = action_from_name(action_name) else { continue };
            for key_str in value.keys() {
                let trimmed = key_str.trim();
                if trimmed.starts_with("prefix+") {
                    let rest = trimmed.strip_prefix("prefix+").unwrap().trim();
                    let combo = keymap::KeyCombo::parse(rest);
                    keymap.bind("normal", combo, action.clone());
                } else {
                    let combo = keymap::KeyCombo::parse(trimmed);
                    keymap.bind("global", combo, action.clone());
                }
            }
        }

        // ── Sidebar-mode bindings (hardcoded for now) ──
        let sidebar_bindings = vec![
            ("j", WmAction::SidebarDown),
            ("k", WmAction::SidebarUp),
            ("h", WmAction::SidebarLeftNav),
            ("l", WmAction::SidebarRightNav),
            ("Tab", WmAction::SidebarExpandToggle),
            ("Space", WmAction::SidebarExpandToggle),
            ("b", WmAction::SidebarLeft),
            ("Enter", WmAction::SidebarRightNav),
        ];
        for (key, action) in sidebar_bindings {
            keymap.bind("sidebar", keymap::KeyCombo::parse(key), action);
        }

        // ── Custom command bindings from [[keys.command]] ──
        for cmd_cfg in &app_config.config.keys.command {
            let action = WmAction::SpawnCommand {
                command: cmd_cfg.command.clone(),
            };
            let trimmed = cmd_cfg.key.trim();
            if trimmed.starts_with("prefix+") {
                let rest = trimmed.strip_prefix("prefix+").unwrap().trim();
                keymap.bind("normal", keymap::KeyCombo::parse(rest), action);
            } else {
                keymap.bind("global", keymap::KeyCombo::parse(trimmed), action);
            }
        }

        // ── Custom modes from [[keys.mode]] ──
        // TODO: implement mode runtime

        Self {
            state: None,
            app_config,
            registry,
            keymap,
        }
    }

    async fn init_state(&mut self, event_loop: &ActiveEventLoop) -> Box<AppState> {
        let window_attrs = Window::default_attributes()
            .with_title("heca")
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.app_config.config.settings.window_width as f64,
                self.app_config.config.settings.window_height as f64,
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
        let fake_pane = LayoutPane::new(PaneId(1), pane_name(1));
        let pane_id = fake_pane.id.0;
        session.add_pane(fake_pane, None, true);

        // Create fake backend (no PTY overhead for layout testing)
        let mut backends: HashMap<u64, Box<dyn PaneBackend>> = HashMap::new();
        backends.insert(pane_id, Box::new(FakeBackend::new(80, 24)));

        let ws_count = session.workspaces.len();
        let mut sidebar_tree = SidebarTree::new();
        sidebar_tree.rebuild(&session, None, Some(pane_id), &vec![None; ws_count]);

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
            swap_and_focus: false,
            mouse_enabled: self.app_config.config.settings.mouse,
            prefix_entered_at: None,
            prefix_combo: keymap::KeyCombo::parse(
                &self.app_config.config.keys.prefix,
            ),
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
            InputMode::Rename { target: _, buffer } => {
                ("RENAME", format!(": {}_", buffer))
            }
            InputMode::Chord { sequence } => {
                ("CHORD", format!(" w→{}", sequence.join("→")))
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
            let name_size = (pw.min(ph) * 0.25).clamp(24.0, 72.0);
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
        let candidates = state.input_mode.candidates();
        if chrome.left_sidebar_width >= 80.0 {
            sidebar::render_sidebar_expanded(
                &state.sidebar_tree,
                0.0, sidebar_top, chrome.left_sidebar_width, sidebar_h,
                matches!(state.input_mode, InputMode::SidebarNav),
                theme.accent.to_f32x4(),
                theme.foreground.to_f32x4(),
                [side_bg[0] * 2.0, side_bg[1] * 2.0, side_bg[2] * 2.0, 0.6],  // cursor highlight
                [theme.accent.to_f32x4()[0], theme.accent.to_f32x4()[1], theme.accent.to_f32x4()[2], 0.5],
                candidates,
                &mut state.text_renderer,
                &mut state.primitive_renderer,
            );
        } else {
            sidebar::render_sidebar_collapsed(
                &state.sidebar_tree,
                0.0, sidebar_top, chrome.left_sidebar_width, sidebar_h,
                matches!(state.input_mode, InputMode::SidebarNav),
                theme.accent.to_f32x4(),
                theme.foreground.to_f32x4(),
                [theme.accent.to_f32x4()[0], theme.accent.to_f32x4()[1], theme.accent.to_f32x4()[2], 0.5],
                [side_bg[0] * 2.0, side_bg[1] * 2.0, side_bg[2] * 2.0, 0.6],
                candidates,
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
                let f_name_size = (fw.min(fh) * 0.25).clamp(24.0, 72.0);
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
            WindowEvent::Resized(phys)
                if phys.width > 0 && phys.height > 0 => {
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
                let _phys = event.physical_key;

                // Build a KeyCombo from the current key event for comparison.
                let event_combo = keymap::KeyCombo {
                    key: key_text.clone(),
                    ctrl: is_ctrl,
                    shift: is_shift,
                    alt: state.modifiers.alt_key(),
                    super_: state.modifiers.super_key(),
                };

                // Check if current key matches the configured prefix combo.
                let is_prefix = event_combo_matches(&event_combo, &state.prefix_combo)
                    // macOS special case: Ctrl+B may report as control char \u{2}
                    || (state.prefix_combo.ctrl && state.prefix_combo.key.eq_ignore_ascii_case("b")
                        && (key_text == "\u{2}"
                            || matches!(log_key, winit::keyboard::Key::Character(c) if c == "\u{2}")));

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
                                if let Some(ws) = state.session.active_workspace_mut()
                                    && let Some(pane) = ws.find_pane_mut(heca_core::layout::PaneId(*pane_id)) {
                                        pane.title = if new_name.is_empty() { format!("pane{}", pane_id) } else { new_name };
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
                            state.prefix_entered_at = Some(std::time::Instant::now());
                            state.needs_redraw = true;
                            return;
                        }

                        // Check global (non-prefix) keybindings before forwarding to terminal.
                        // These use modifiers (Alt, Super, F-keys) to avoid stealing typing.
                        let global_action = self.keymap.resolve("global", &event_combo).cloned();
                        if let Some(act) = global_action {
                            self.registry.execute(&act, state);
                            return;
                        }

                        // Forward key to focused pane's backend (terminal input)
                        if let Some(pane_id) = state.focused_pane
                            && let Some(backend) = state.backends.get_mut(&pane_id) {
                                let input_bytes = winit_key_to_terminal_input(
                                    &event.logical_key,
                                    &key_text,
                                    is_ctrl,
                                );
                                if !input_bytes.is_empty() {
                                    backend.process_input(&input_bytes);
                                }
                            }
                    }
                    InputMode::Prefix => {
                        if is_prefix {
                            // Double-prefix: forward literal Ctrl+B (0x02) to the terminal
                            // so nested tmux/screen work correctly.
                            state.input_mode = InputMode::Normal;
                            state.prefix_entered_at = None;
                            if let Some(pane_id) = state.focused_pane
                                && let Some(backend) = state.backends.get_mut(&pane_id) {
                                    backend.process_input(&[0x02]);
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
                        let combo = keymap::KeyCombo { key: key_text.clone(), ctrl: is_ctrl, shift: is_shift, alt: false, super_: false };

                        // Check for chord starters (e.g. "w" starts workspace-switch chord).
                        if key_text.eq_ignore_ascii_case("w") && !is_ctrl && !is_shift {
                            state.input_mode = InputMode::Chord { sequence: vec!["w".to_string()] };
                            state.prefix_entered_at = Some(std::time::Instant::now());
                            state.needs_redraw = true;
                            return;
                        }

                        let action = self.keymap.resolve("normal", &combo).cloned();
                        // Only reset to Normal if we found an action or the key is printable.
                        // If no action matched and key_text is empty, stay in prefix (e.g. dead keys).
                        if let Some(ref act) = action {
                            state.input_mode = InputMode::Normal;
                            state.prefix_entered_at = None;
                            self.registry.execute(act, state);
                        } else if !key_text.is_empty() {
                            // Printable key that didn't match any binding — exit prefix.
                            state.input_mode = InputMode::Normal;
                            state.prefix_entered_at = None;
                        }
                        // else: empty key_text, no action — stay in prefix mode.
                    }
                    InputMode::Chord { sequence } => {
                        let is_escape = matches!(event.logical_key, winit::keyboard::Key::Named(NamedKey::Escape));
                        if is_escape {
                            state.input_mode = InputMode::Normal;
                            state.needs_redraw = true;
                            return;
                        }

                        // Hardcoded chord: w → digit switches to workspace.
                        if sequence.len() == 1
                            && sequence[0].eq_ignore_ascii_case("w")
                            && let Some(digit) = key_text.chars().next()
                                .filter(|c| c.is_ascii_digit())
                                .and_then(|c| c.to_digit(10))
                        {
                            let ws_idx = (digit as usize).saturating_sub(1);
                            if ws_idx < state.session.workspaces.len() {
                                switch_workspace_tracked(state, ws_idx);
                                sync_focus(state);
                                state.needs_redraw = true;
                            }
                            state.input_mode = InputMode::Normal;
                            return;
                        }

                        // Unknown chord key — cancel.
                        state.input_mode = InputMode::Normal;
                        state.needs_redraw = true;
                    }
                    InputMode::PaneSelect { candidates } => {
                        let candidates = candidates.clone();
                        state.input_mode = InputMode::Normal;
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
                        if let Some(ch) = typed
                            && let Some((_, target_id)) = candidates.iter().find(|(c, _)| *c == ch) {
                                focus_pane_by_id(state, *target_id);
                            }
                        state.input_mode = InputMode::Normal;
                    }
                    InputMode::PaneSwap { candidates } => {
                        // Copy candidates and swap_and_focus so we can mutate state
                        let candidates = candidates.clone();
                        let should_focus = state.swap_and_focus;
                        state.swap_and_focus = false;
                        state.input_mode = InputMode::Normal;

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
                        if let Some(ch) = typed {
                            let mut moved_pane: Option<u64> = None;
                            if let Some((_, target_id)) = candidates.iter().find(|(c, _)| *c == ch)
                                && let Some(current_id) = state.focused_pane {
                                    let current_col = find_pane_column(&state.session, current_id);
                                    let target_col = find_pane_column(&state.session, *target_id);
                                    if let (Some((cws, ccol)), Some((tws, tcol))) = (current_col, target_col) {
                                        if cws == tws && ccol == tcol {
                                            // Same column — no-op
                                        } else if cws == tws {
                                            // Same workspace, different column: move pane to target column
                                            move_pane_to_column(state, current_id, ccol, tcol);
                                            moved_pane = Some(current_id);
                                        } else {
                                            // Cross-workspace: move pane to target workspace, target column
                                            move_pane_to_workspace_column(state, current_id, tws, tcol);
                                            moved_pane = Some(current_id);
                                        }
                                    }
                                }
                            if should_focus
                                && let Some(pane_id) = moved_pane {
                                    focus_pane_by_id(state, pane_id);
                                }
                        }
                        state.needs_redraw = true;
                    }
                    InputMode::SidebarNav => {
                        let is_escape = matches!(event.logical_key, winit::keyboard::Key::Named(NamedKey::Escape));

                        if is_escape {
                            state.input_mode = InputMode::Normal;
                            state.needs_redraw = true;
                        } else {
                            let combo = keymap::KeyCombo { key: key_text.clone(), ctrl: is_ctrl, shift: is_shift, alt: false, super_: false };
                            let action = self.keymap.resolve("sidebar", &combo).cloned();
                            if let Some(act) = action {
                                self.registry.execute(&act, state);
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

                if !state.mouse_enabled {}
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
                    // ── Left sidebar hit test ──
                    let sidebar_top = chrome.tab_bar_height;
                    let sidebar_bottom = win_h - chrome.status_bar_height;
                    let sidebar_h = sidebar_bottom - sidebar_top;
                    if mouse_pos.0 >= 0.0 && mouse_pos.0 <= chrome.left_sidebar_width
                        && mouse_pos.1 >= sidebar_top && mouse_pos.1 <= sidebar_bottom
                    {
                        if let Some(fi) = sidebar::sidebar_hit_test(
                            &state.sidebar_tree,
                            sidebar_top, sidebar_h, chrome.left_sidebar_width,
                            mouse_pos.1,
                        ) {
                            state.sidebar_tree.cursor = fi;
                            state.input_mode = InputMode::SidebarNav;
                            // Activate the clicked item (same as Enter in SidebarNav)
                            let item = state.sidebar_tree.current_item().cloned();
                            match &item {
                                Some(sidebar::SidebarItem::Pane { pane_id }) => {
                                    let target_pane_id = heca_core::layout::PaneId(*pane_id);
                                    let target_ws = state.session.workspaces.iter().position(|ws| {
                                        ws.find_pane(target_pane_id).is_some()
                                    });
                                    if let Some(ws_idx) = target_ws {
                                        if ws_idx != state.session.active_workspace_idx {
                                            switch_workspace_tracked(state, ws_idx);
                                        }
                                        focus_pane_by_id(state, *pane_id);
                                    }
                                    state.input_mode = InputMode::Normal;
                                }
                                Some(sidebar::SidebarItem::Workspace { .. }) => {
                                    let ws_idx = state.sidebar_tree.cursor_workspace_index()
                                        .unwrap_or(state.session.active_workspace_idx);
                                    if ws_idx != state.session.active_workspace_idx {
                                        switch_workspace_tracked(state, ws_idx);
                                    }
                                    sync_focus(state);
                                    state.input_mode = InputMode::Normal;
                                }
                                _ => {}
                            }
                        }
                        return;
                    }

                    // ── Pane content area hit test ──
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
                            focus_pane_by_id(state, pane_id.0);
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
            // Prefix / Chord mode auto-timeout: exit if inactive > 500 ms.
            let should_timeout = matches!(state.input_mode, InputMode::Prefix | InputMode::Chord { .. })
                && state.prefix_entered_at.is_some_and(|entered| entered.elapsed() >= Duration::from_millis(500));
            if should_timeout {
                state.input_mode = InputMode::Normal;
                state.prefix_entered_at = None;
                state.needs_redraw = true;
            }

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

/// Find which workspace contains a pane (by ID). Returns workspace index or None.
fn find_pane_workspace(session: &Session, pane_id: u64) -> Option<usize> {
    let target = heca_core::layout::PaneId(pane_id);
    session.workspaces.iter().position(|ws| {
        ws.find_pane(target).is_some()
    })
}

/// Collect ALL panes across ALL workspaces as letter candidates.
/// Hard-capped at 52 unique labels (a–z, A–Z). Beyond that, use sidebar
/// navigation instead of letter selection.
pub(crate) fn collect_all_pane_candidates(session: &Session) -> Vec<(char, u64)> {
    let mut candidates = Vec::new();
    for ws in &session.workspaces {
        for col in &ws.scrolling.columns {
            for pane in &col.panes {
                if candidates.len() >= CANDIDATE_ALPHABET.len() {
                    return candidates;
                }
                let ch = CANDIDATE_ALPHABET[candidates.len()];
                candidates.push((ch, pane.id.0));
            }
        }
        for float in &ws.floating_panes {
            if candidates.len() >= CANDIDATE_ALPHABET.len() {
                return candidates;
            }
            let ch = CANDIDATE_ALPHABET[candidates.len()];
            candidates.push((ch, float.pane.id.0));
        }
    }
    candidates
}

/// Find the (workspace_index, column_index) containing a pane.
pub(crate) fn find_pane_column(session: &Session, pane_id: u64) -> Option<(usize, usize)> {
    let target = heca_core::layout::PaneId(pane_id);
    for (ws_idx, ws) in session.workspaces.iter().enumerate() {
        for (col_idx, col) in ws.scrolling.columns.iter().enumerate() {
            if col.panes.iter().any(|p| p.id == target) {
                return Some((ws_idx, col_idx));
            }
        }
    }
    None
}

/// Find the (workspace_index, column_index, pane_index) containing a pane.
pub(crate) fn find_pane_location(session: &Session, pane_id: u64) -> Option<(usize, usize, usize)> {
    let target = heca_core::layout::PaneId(pane_id);
    for (ws_idx, ws) in session.workspaces.iter().enumerate() {
        for (col_idx, col) in ws.scrolling.columns.iter().enumerate() {
            if let Some(pane_idx) = col.panes.iter().position(|p| p.id == target) {
                return Some((ws_idx, col_idx, pane_idx));
            }
        }
    }
    None
}

/// Collect ALL columns across ALL workspaces as letter candidates.
/// Hard-capped at 52 unique labels (a–z, A–Z).
/// The candidate ID is the first pane ID in the column (used for positioning the letter overlay).
pub(crate) fn collect_all_column_candidates(session: &Session) -> Vec<(char, u64)> {
    let mut candidates = Vec::new();
    for ws in &session.workspaces {
        for col in &ws.scrolling.columns {
            if candidates.len() >= CANDIDATE_ALPHABET.len() {
                return candidates;
            }
            if let Some(first_pane) = col.panes.first() {
                let ch = CANDIDATE_ALPHABET[candidates.len()];
                candidates.push((ch, first_pane.id.0));
            }
        }
    }
    candidates
}

/// The single canonical way to focus a pane.
///
/// 1. Switches workspace if the pane is in a different workspace.
/// 2. Activates the pane in the session (scrolling column + pane indices).
/// 3. Calls `sync_focus()` to update `focused_pane`, track history, and rebuild the sidebar.
///
/// **All** focus changes must go through this function — mouse clicks, keyboard nav,
/// sidebar selection, pane select, swap-and-focus, etc.
pub(crate) fn focus_pane_by_id(state: &mut AppState, pane_id: u64) {
    // Switch workspace if the target pane is not in the current workspace.
    if let Some(target_ws) = find_pane_workspace(&state.session, pane_id)
        && target_ws != state.session.active_workspace_idx {
            switch_workspace_tracked(state, target_ws);
        }

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
            ws.scrolling.activate_column(ci);
            if let Some(col) = ws.scrolling.columns.get_mut(ci) {
                col.activate_pane(pi);
            }
        } else {
            // Not in scrolling columns — check floating panes.
            for float in &mut ws.floating_panes {
                if float.pane.id.0 == pane_id {
                    ws.floating_is_active = true;
                    float.is_active = true;
                    break;
                }
            }
        }
    }

    // sync_focus reads the session's active pane, updates AppState, records history,
    // and rebuilds the sidebar tree. This is the ONLY place the sidebar rebuilds.
    sync_focus(state);
}

/// Sync `focused_pane` from the session's active pane (scrolling or floating).
/// Also tracks last-visited workspace and rebuilds the sidebar tree.
/// The single canonical way to switch workspaces.
/// Updates last_visited_ws_idx so Prefix+Shift+l can toggle back.
/// Does NOT touch last_visited_pane_per_ws — that field is reserved for
/// same-workspace pane toggle (Prefix+i) and must not be overwritten by
/// workspace switches.
pub(crate) fn switch_workspace_tracked(state: &mut AppState, new_idx: usize) {
    let current_ws = state.session.active_workspace_idx;
    if current_ws == new_idx {
        return;
    }
    state.last_visited_ws_idx = Some(current_ws);
    state.session.switch_to_workspace(new_idx);
}

pub(crate) fn sync_focus(state: &mut AppState) {
    let prev_focused = state.focused_pane;
    let prev_ws = state.session.active_workspace_idx;

    // Update focused_pane from session state
    state.focused_pane = state.session.active_workspace()
        .and_then(|ws| ws.active_pane())
        .map(|p| p.id.0);

    let focus_changed = prev_focused != state.focused_pane;
    let current_ws = state.session.active_workspace_idx;

    // Only record per-workspace data for same-workspace focus changes.
    // If prev_focused doesn't belong to current_ws, a workspace switch
    // happened and switch_workspace_tracked() already recorded it.
    let pane_belongs_to_current_ws = prev_focused.is_some_and(|pid| {
        state.session.workspaces.get(current_ws)
            .map(|ws| ws.find_pane(heca_core::layout::PaneId(pid)).is_some())
            .unwrap_or(false)
    });
    if focus_changed && prev_focused.is_some() && prev_ws == current_ws && pane_belongs_to_current_ws {
        while state.last_visited_pane_per_ws.len() <= current_ws {
            state.last_visited_pane_per_ws.push(None);
        }
        state.last_visited_pane_per_ws[current_ws] = prev_focused;
    }

    // Track global last_focused (for Prefix+Shift+l toggle)
    if focus_changed && prev_focused.is_some() {
        state.last_focused = prev_focused;
    }

    // Rebuild sidebar tree
    state.sidebar_tree.rebuild(
        &state.session,
        state.last_visited_ws_idx,
        state.focused_pane,
        &state.last_visited_pane_per_ws,
    );
}

/// Update session viewport to match current chrome/content area size.
pub(crate) fn update_session_viewport(state: &mut AppState) {
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

/// Build the action registry and register individual handlers for all actions.
pub fn build_registry() -> actions::ActionRegistry {
    use actions::ActionRegistry;
    use handlers::*;
    use input::WmAction;

    let mut registry = ActionRegistry::new();

    // ── Navigation ──
    registry.register(&WmAction::FocusLeft, handle_focus_left);
    registry.register(&WmAction::FocusRight, handle_focus_right);
    registry.register(&WmAction::FocusUp, handle_focus_up);
    registry.register(&WmAction::FocusDown, handle_focus_down);
    registry.register(&WmAction::NextPane, handle_next_pane);
    registry.register(&WmAction::PrevPane, handle_prev_pane);
    registry.register(&WmAction::WorkspaceNext, handle_workspace_next);
    registry.register(&WmAction::WorkspacePrev, handle_workspace_prev);
    registry.register(&WmAction::FocusToggleLocal, handle_focus_toggle_local);
    registry.register(&WmAction::FocusToggleGlobal, handle_focus_toggle_global);
    registry.register(&WmAction::FocusPane { pane_id: 0 }, handle_focus_pane);
    registry.register(&WmAction::FocusWorkspace { ws_idx: 0 }, handle_focus_workspace);

    // ── Layout ──
    registry.register(&WmAction::SplitHorizontal, handle_split_horizontal);
    registry.register(&WmAction::SplitVertical, handle_split_vertical);
    registry.register(&WmAction::ResizeIncrease, handle_resize_increase);
    registry.register(&WmAction::ResizeDecrease, handle_resize_decrease);
    registry.register(&WmAction::PaneHeightIncrease, handle_pane_height_increase);
    registry.register(&WmAction::PaneHeightDecrease, handle_pane_height_decrease);
    registry.register(&WmAction::SwapLeft, handle_swap_left);
    registry.register(&WmAction::SwapRight, handle_swap_right);
    registry.register(&WmAction::SwapUp, handle_swap_up);
    registry.register(&WmAction::SwapDown, handle_swap_down);
    registry.register(&WmAction::MovePaneLeft, handle_move_pane_left);
    registry.register(&WmAction::MovePaneRight, handle_move_pane_right);
    registry.register(&WmAction::Swap { a_id: 0, b_id: 0 }, handle_swap_param);
    registry.register(&WmAction::Move { pane_id: 0, target_col: 0 }, handle_move_param);
    registry.register(&WmAction::Resize { target: input::ResizeTarget::Column, delta: 0 }, handle_resize);
    registry.register(&WmAction::ResizeTo { target: input::ResizeTarget::Column, width: 0.0, height: 0.0 }, handle_resize_to);

    // ── Pane ──
    registry.register(&WmAction::Float, handle_float);
    registry.register(&WmAction::ClosePane, handle_close_pane);
    registry.register(&WmAction::PaneSelect, handle_pane_select);
    registry.register(&WmAction::SwapSelect, handle_swap_select);
    registry.register(&WmAction::SwapAndFocus, handle_swap_and_focus);
    registry.register(&WmAction::RenamePane, handle_rename_pane);
    registry.register(&WmAction::FloatAt { pane_id: 0, x: 0.0, y: 0.0, width: 0.0, height: 0.0 }, handle_float_at);
    registry.register(&WmAction::ClosePaneById { pane_id: 0 }, handle_close_pane_by_id);
    registry.register(&WmAction::RenameTarget { pane_id: 0, name: String::new() }, handle_rename_target);

    // ── Workspace ──
    registry.register(&WmAction::CreateWorkspace, handle_create_workspace);
    registry.register(&WmAction::RenameWorkspace, handle_rename_workspace);

    // ── Sidebar / Chrome ──
    registry.register(&WmAction::SidebarLeft, handle_sidebar_left);
    registry.register(&WmAction::SidebarRight, handle_sidebar_right);
    registry.register(&WmAction::SidebarFocus, handle_sidebar_focus);
    registry.register(&WmAction::SidebarUp, handle_sidebar_up);
    registry.register(&WmAction::SidebarDown, handle_sidebar_down);
    registry.register(&WmAction::SidebarLeftNav, handle_sidebar_left_nav);
    registry.register(&WmAction::SidebarRightNav, handle_sidebar_right_nav);
    registry.register(&WmAction::SidebarExpandToggle, handle_sidebar_expand_toggle);
    registry.register(&WmAction::TabNext, handle_tab_next);
    registry.register(&WmAction::TabPrev, handle_tab_prev);

    // ── System ──
    registry.register(&WmAction::CommandPalette, handle_command_palette);
    registry.register(&WmAction::SpawnCommand { command: String::new() }, handle_spawn_command);

    registry
}
pub(crate) fn move_pane_to_workspace_column(state: &mut AppState, pane_id: u64, target_ws: usize, target_col: usize) {
    let current_ws = state.session.active_workspace_idx;
    if current_ws == target_ws {
        return;
    }

    let removed_pane = {
        let ws = match state.session.workspaces.get_mut(current_ws) {
            Some(ws) => ws,
            None => return,
        };
        let mut removed = None;
        for ci in 0..ws.scrolling.columns.len() {
            if let Some(pi) = ws.scrolling.columns[ci].panes.iter().position(|p| p.id.0 == pane_id) {
                removed = ws.scrolling.remove_pane(ci, pi);
                break;
            }
        }
        removed
    };

    if let Some(pane) = removed_pane {
        state.session.switch_to_workspace(target_ws);

        // Add to target column if it exists, otherwise create a new column.
        if let Some(ws) = state.session.active_workspace_mut() {
            if target_col < ws.scrolling.columns.len() {
                ws.scrolling.add_pane_to_column(target_col, None, pane, true);
            } else {
                state.session.add_pane(pane, None, true);
            }
            state.focused_pane = Some(pane_id);
        }

        destroy_empty_workspace(state, current_ws);
    }

    sync_focus(state);
}

/// Move a pane from one column to another within the same workspace.
/// Handles column removal when a column becomes empty after the move.
pub(crate) fn move_pane_to_column(state: &mut AppState, pane_id: u64, src_col: usize, dst_col: usize) {
    if src_col == dst_col { return; }

    let ws_idx = state.session.active_workspace_idx;

    // Validate indices before mutation.
    let col_count_before = state.session.workspaces.get(ws_idx)
        .map(|ws| ws.scrolling.columns.len())
        .unwrap_or(0);
    if src_col >= col_count_before || dst_col >= col_count_before {
        return;
    }

    let removed_pane = if let Some(ws) = state.session.workspaces.get_mut(ws_idx) {
        if let Some(pi) = ws.scrolling.columns.get(src_col)
            .and_then(|col| col.panes.iter().position(|p| p.id.0 == pane_id))
        {
            ws.scrolling.remove_pane(src_col, pi)
        } else {
            None
        }
    } else {
        None
    };

    if let Some(pane) = removed_pane {
        // Check if the source column was removed (became empty after removal).
        let col_count_after = state.session.workspaces.get(ws_idx)
            .map(|ws| ws.scrolling.columns.len())
            .unwrap_or(0);
        let col_removed = col_count_after < col_count_before;

        let adjusted_dst = if col_removed && src_col < dst_col {
            dst_col.saturating_sub(1)
        } else {
            dst_col
        };

        if let Some(ws) = state.session.workspaces.get_mut(ws_idx) {
            let target_col = adjusted_dst.min(ws.scrolling.columns.len().saturating_sub(1));
            if target_col < ws.scrolling.columns.len() {
                ws.scrolling.add_pane_to_column(target_col, None, pane, true);
            } else {
                ws.scrolling.add_column(None, Column::new(
                    ColumnId(pane.id.0), pane, ColumnWidth::Proportion(0.5),
                ), true);
            }
            state.focused_pane = Some(pane_id);
        }
    }

    sync_focus(state);
}

/// Remove a workspace if it is empty and there are other workspaces.
/// Adjusts tracking indices after removal.
pub(crate) fn destroy_empty_workspace(state: &mut AppState, ws_idx: usize) {
    let is_empty = state.session.workspaces.get(ws_idx)
        .map(|ws| ws.scrolling.columns.iter().all(|c| c.panes.is_empty()))
        .unwrap_or(true);

    if is_empty && state.session.workspaces.len() > 1 {
        state.session.remove_workspace(ws_idx);

        // Fix up last_visited_ws_idx if it pointed to the removed workspace.
        if state.last_visited_ws_idx == Some(ws_idx) {
            state.last_visited_ws_idx = None;
        } else if let Some(ref mut idx) = state.last_visited_ws_idx
            && *idx > ws_idx {
                *idx -= 1;
            }

        // Fix up last_visited_pane_per_ws — remove the entry for the removed workspace.
        if ws_idx < state.last_visited_pane_per_ws.len() {
            state.last_visited_pane_per_ws.remove(ws_idx);
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = HecaApp::new();
    event_loop.run_app(&mut app).unwrap();
}
