mod actions;
mod app;
mod app_state;
mod chrome;
mod handlers;
mod input;
mod keymap;
mod mouse;
mod rpc;
mod sidebar;

use sidebar::SidebarTree;

pub(crate) use app::focus::{focus_pane_by_id, switch_workspace_tracked, sync_focus};
use app::keyboard::{event_combo_matches, normalize_key_text, winit_key_to_terminal_input};
pub(crate) use app::mutations::{
    destroy_empty_workspace, move_column_to_workspace, move_pane_to_column,
    move_pane_to_workspace_column,
};
use app::registry::{build_keymap, build_modes, build_registry};
use app::render::render_backend_data;
pub(crate) use app::render::update_session_viewport;
pub(crate) use app::selection::{collect_all_pane_candidates, find_pane_location};
use app_state::{AppState, DragState, InputMode, RenameTarget, SidebarState};
use chrome::ChromeConfig;
use heca_config::theme::AppConfig;
use heca_core::backend::{FakeBackend, PaneBackend};
use heca_core::layout::{Pane as LayoutPane, PaneId, Session};
use heca_renderer::primitive::PrimitiveRenderer;
use heca_renderer::text::TextRenderer;
use input::WmAction;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::keyboard::NamedKey;

use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};

use winit::window::{Window, WindowId};

/// Distinct pane names so you can visually identify what's moving.
const PANE_NAMES: &[&str] = &[
    "Red", "Green", "Blue", "Yellow", "Cyan", "Magenta", "Orange", "Purple", "Lime", "Pink",
    "Teal", "Coral",
];

pub(crate) fn pane_name(id: u64) -> String {
    PANE_NAMES
        .get((id as usize).saturating_sub(1) % PANE_NAMES.len())
        .unwrap_or(&"?")
        .to_string()
}

struct HecaApp {
    state: Option<Box<AppState>>,
    app_config: AppConfig,
    registry: actions::ActionRegistry,
    keymap: keymap::KeymapRegistry,
    /// Per-mode keymaps (e.g. "resize" mode bindings).
    mode_keymaps: HashMap<String, keymap::KeymapRegistry>,
    /// Mode triggers: name → (trigger combo, sticky).
    /// Populated from [[keys.mode]] trigger field.
    mode_triggers: HashMap<String, (keymap::KeyCombo, bool)>,
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
        let keymap = build_keymap(&app_config.config);
        let (mode_keymaps, mode_triggers) = build_modes(&app_config.config);

        Self {
            state: None,
            app_config,
            registry,
            keymap,
            mode_keymaps,
            mode_triggers,
        }
    }

    fn reload_config(&mut self) {
        if let Some(ref mut state) = self.state {
            let new_config = AppConfig::load();
            self.app_config = new_config.clone();
            self.keymap = build_keymap(&self.app_config.config);
            let (new_mode_keymaps, new_mode_triggers) = build_modes(&self.app_config.config);
            self.mode_keymaps = new_mode_keymaps;
            self.mode_triggers = new_mode_triggers;
            state.theme = self.app_config.theme.clone();
            state.prefix_combo = keymap::KeyCombo::parse(&self.app_config.config.keys.prefix);
            state.mouse_enabled = self.app_config.config.settings.mouse;
            state.auto_scroll_edge = self.app_config.config.settings.auto_scroll_edge;
            state.interactive_move_modifier =
                self.app_config.config.settings.interactive_move_modifier;
            state.needs_redraw = true;
        }
    }

    async fn init_state(&mut self, event_loop: &ActiveEventLoop) -> Box<AppState> {
        let window_attrs = Window::default_attributes()
            .with_title("heca")
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.app_config.config.settings.window_width as f64,
                self.app_config.config.settings.window_height as f64,
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
        text_renderer.set_font_family(&self.app_config.theme.font_family);
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
        let viewport_size =
            heca_core::layout::types::Size::new(pane_area.w as f64, pane_area.h as f64);
        let layout_options = heca_core::layout::types::LayoutOptions {
            always_center_single_column: self
                .app_config
                .config
                .settings
                .always_center_single_column,
            ..Default::default()
        };
        let mut session = Session::new(
            heca_core::layout::types::SessionId(1),
            viewport_size,
            scale_factor,
            layout_options,
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
            mouse: app_state::MouseState::new(),
            modifiers: winit::keyboard::ModifiersState::default(),
            last_focused: None,
            last_visited_ws_idx: None,
            last_visited_pane_per_ws: vec![None; ws_count],
            mouse_enabled: self.app_config.config.settings.mouse,
            auto_scroll_edge: self.app_config.config.settings.auto_scroll_edge,
            interactive_move_modifier: self.app_config.config.settings.interactive_move_modifier,
            prefix_entered_at: None,
            prefix_combo: keymap::KeyCombo::parse(&self.app_config.config.keys.prefix),
            pending_reload: false,
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
                state
                    .surface
                    .configure(&state.device, &state.surface_config);
                return;
            }
            Err(wgpu::SurfaceError::OutOfMemory) => std::process::exit(1),
            Err(_) => {
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
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render"),
            });

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
        state
            .primitive_renderer
            .draw_rect(0.0, 0.0, w, tb.tab_bar_height, side_bg);
        for (i, tab_name) in state.tab_names.iter().enumerate() {
            let tab_x = 4.0 + i as f32 * 120.0;
            let tab_color = if i == state.active_tab {
                theme.accent.to_f32x4()
            } else {
                theme.border.to_f32x4()
            };
            state.primitive_renderer.draw_rect(
                tab_x,
                2.0,
                116.0,
                tb.tab_bar_height - 4.0,
                tab_color,
            );
            let tab_text_y = (tb.tab_bar_height - chrome_text) / 2.0;
            state.text_renderer.queue_text(
                tab_name,
                tab_x + 4.0,
                tab_text_y,
                chrome_text,
                theme.foreground.to_f32x4(),
            );
        }

        // ── STATUS BAR ──
        let sb_y = h - tb.status_bar_height;
        state
            .primitive_renderer
            .draw_rect(0.0, sb_y, w, tb.status_bar_height, side_bg);
        let pane_count = state
            .session
            .active_workspace()
            .map(|ws| {
                ws.scrolling
                    .columns
                    .iter()
                    .map(|c| c.panes.len())
                    .sum::<usize>()
            })
            .unwrap_or(0);
        let focus_title = state
            .session
            .active_workspace()
            .and_then(|ws| ws.scrolling.active_pane())
            .map(|p| p.title.as_str())
            .unwrap_or("—");
        let (mode_str, rename_hint) = match &state.input_mode {
            InputMode::Normal => ("NORMAL", String::new()),
            InputMode::Prefix => ("PREFIX", String::new()),
            InputMode::PaneSelect { .. } => ("SELECT", String::new()),
            InputMode::PaneSwap { focus_after, .. } => {
                if *focus_after {
                    ("SWAP+FOCUS", String::new())
                } else {
                    ("SWAP", String::new())
                }
            }
            InputMode::SidebarNav => ("SIDEBAR", String::new()),
            InputMode::Rename { target: _, buffer } => ("RENAME", format!(": {}_", buffer)),
            InputMode::Chord { sequence } => ("CHORD", format!(" w→{}", sequence.join("→"))),
            InputMode::Mode { name } => ("MODE", format!(" {} → ?", name)),
            InputMode::ConfirmDelete { message, .. } => ("CONFIRM", format!(" {} ", message)),
            InputMode::PaneTake { focus_after, .. } => {
                if *focus_after {
                    ("TAKE+", " pick a pane → ".to_string())
                } else {
                    ("TAKE", " pick a pane → ".to_string())
                }
            }
        };
        let status = format!(
            "{} panes | {} | {}{}",
            pane_count, focus_title, mode_str, rename_hint
        );
        let status_text_y = sb_y + (tb.status_bar_height - chrome_text) / 2.0;
        state.text_renderer.queue_text(
            &status,
            8.0,
            status_text_y,
            chrome_text,
            theme.foreground.to_f32x4(),
        );

        // ── Flush tab bar + status bar ──
        state
            .primitive_renderer
            .render(&state.device, &view, &mut encoder);
        state
            .text_renderer
            .render(&state.device, &state.queue, &view, &mut encoder);

        // ── PANE CONTENT AREA ──
        let theme_border = theme.border.to_f32x4();
        let border_width = theme.border_width;
        let accent_color = theme.accent.to_f32x4();

        // Get pane positions from NIRI layout engine
        let pane_positions = state
            .session
            .active_workspace()
            .map(|ws| ws.scrolling.panes_with_positions())
            .unwrap_or_default();

        // Get workspace geometry for overview (normal mode = full size)
        let ws_geometries = state.session.workspace_geometries();
        let ws_offset = ws_geometries
            .first()
            .map(|(_, rect)| (rect.loc.x as f32, rect.loc.y as f32))
            .unwrap_or((0.0, 0.0));

        // ── PANES ──
        for (pane_id, rect) in &pane_positions {
            let px = pane_area.x + ws_offset.0 + rect.loc.x as f32;
            let py = pane_area.y + ws_offset.1 + rect.loc.y as f32;
            let pw = rect.size.w as f32;
            let ph = rect.size.h as f32;
            let is_active = state.focused_pane == Some(pane_id.0);
            let bcolor = if is_active {
                accent_color
            } else {
                [theme_border[0], theme_border[1], theme_border[2], 0.5]
            };

            if let Some(backend) = state.backends.get(&pane_id.0) {
                let data = backend.render_data();
                render_backend_data(
                    &data,
                    px,
                    py,
                    pw,
                    ph,
                    &mut state.text_renderer,
                    &mut state.primitive_renderer,
                    theme,
                );
            } else {
                state
                    .primitive_renderer
                    .draw_rect(px, py, pw, ph, [0.118, 0.118, 0.180, 1.0]);
            }

            // Draw pane name as large centered label so you can tell panes apart
            let pane_name = state
                .session
                .active_workspace()
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
            state
                .text_renderer
                .queue_text(pane_name, name_x, name_y, name_size, name_color);

            state
                .primitive_renderer
                .draw_border(px, py, pw, ph, bcolor, border_width);
        }
        state
            .primitive_renderer
            .render(&state.device, &view, &mut encoder);
        state
            .text_renderer
            .render(&state.device, &state.queue, &view, &mut encoder);

        // ── SIDEBARS (drawn ON TOP of panes so they cover any overflow) ──
        let sidebar_top = chrome.tab_bar_height;
        let sidebar_bottom = h - chrome.status_bar_height;
        let sidebar_h = sidebar_bottom - sidebar_top;

        // Left sidebar
        state.primitive_renderer.draw_rect(
            0.0,
            sidebar_top,
            chrome.left_sidebar_width,
            sidebar_h,
            side_bg,
        );
        state.primitive_renderer.draw_border(
            chrome.left_sidebar_width - 1.0,
            sidebar_top,
            1.0,
            sidebar_h,
            theme.border.to_f32x4(),
            1.0,
        );
        let candidates = state.input_mode.candidates();
        let drag_hover_fi = state.mouse.drag_hover_sidebar_fi;
        let drag_source_fi = state.mouse.sidebar_drag_source_fi;
        let drag_source_bg = theme.sidebar_drag_source_bg.to_f32x4();
        let drag_source_border = theme.sidebar_drag_source_border.to_f32x4();
        if chrome.left_sidebar_width >= 80.0 {
            sidebar::render_sidebar_expanded(
                &mut state.sidebar_tree,
                0.0,
                sidebar_top,
                chrome.left_sidebar_width,
                sidebar_h,
                matches!(state.input_mode, InputMode::SidebarNav),
                theme.accent.to_f32x4(),
                theme.foreground.to_f32x4(),
                [side_bg[0] * 2.0, side_bg[1] * 2.0, side_bg[2] * 2.0, 0.6], // cursor highlight
                [
                    theme.accent.to_f32x4()[0],
                    theme.accent.to_f32x4()[1],
                    theme.accent.to_f32x4()[2],
                    0.5,
                ],
                candidates,
                state.focused_pane,
                &mut state.text_renderer,
                &mut state.primitive_renderer,
                drag_hover_fi,
                drag_source_fi,
                drag_source_bg,
                drag_source_border,
                state.mouse.sidebar_hovered_btn_idx,
                theme.sidebar_label_font_size,
                theme.sidebar_button_font_size,
            );
        } else {
            sidebar::render_sidebar_collapsed(
                &mut state.sidebar_tree,
                0.0,
                sidebar_top,
                chrome.left_sidebar_width,
                sidebar_h,
                matches!(state.input_mode, InputMode::SidebarNav),
                theme.accent.to_f32x4(),
                theme.foreground.to_f32x4(),
                [
                    theme.accent.to_f32x4()[0],
                    theme.accent.to_f32x4()[1],
                    theme.accent.to_f32x4()[2],
                    0.5,
                ],
                [side_bg[0] * 2.0, side_bg[1] * 2.0, side_bg[2] * 2.0, 0.6],
                candidates,
                state.focused_pane,
                &mut state.text_renderer,
                &mut state.primitive_renderer,
                drag_hover_fi,
                drag_source_fi,
                drag_source_bg,
                drag_source_border,
                state.mouse.sidebar_hovered_btn_idx,
                theme.sidebar_label_font_size,
                theme.sidebar_button_font_size,
            );
        }

        // Render ghost label during sidebar drag.
        if let Some(label) = &state.mouse.sidebar_drag_label {
            let ghost_w = label.width;
            let ghost_h = 22.0;
            let ghost_x = label.x + 10.0; // offset from cursor
            let ghost_y = label.y - ghost_h / 2.0; // center on cursor

            // Ghost background from theme.
            state.primitive_renderer.draw_rect(
                ghost_x,
                ghost_y,
                ghost_w,
                ghost_h,
                theme.sidebar_drag_ghost_bg.to_f32x4(),
            );

            // Ghost border from theme.
            state.primitive_renderer.draw_border(
                ghost_x,
                ghost_y,
                ghost_w,
                ghost_h,
                theme.sidebar_drag_source_border.to_f32x4(),
                1.5,
            );

            // Ghost text from theme.
            state.text_renderer.queue_text(
                &label.text,
                ghost_x + 6.0,
                ghost_y + 4.0,
                13.0,
                theme.sidebar_drag_ghost_fg.to_f32x4(),
            );
        }

        // Right sidebar
        let rsx = w - chrome.right_sidebar_width;
        state.primitive_renderer.draw_rect(
            rsx,
            sidebar_top,
            chrome.right_sidebar_width,
            sidebar_h,
            side_bg,
        );
        state.primitive_renderer.draw_border(
            rsx,
            sidebar_top,
            1.0,
            sidebar_h,
            theme.border.to_f32x4(),
            1.0,
        );
        if chrome.right_sidebar_width >= 80.0 {
            state.text_renderer.queue_text(
                "Details",
                rsx + 8.0,
                sidebar_top + 8.0,
                chrome_text,
                theme.foreground.to_f32x4(),
            );
        }

        // ── FLOATING PANES ──
        if let Some(ws) = state.session.active_workspace() {
            for float in &ws.floating_panes {
                let fx = float.position.x as f32 + pane_area.x + ws_offset.0;
                let fy = float.position.y as f32 + pane_area.y + ws_offset.1;
                let fw = float.size.w as f32;
                let fh = float.size.h as f32;
                let is_focused = state.focused_pane == Some(float.pane.id.0);
                let fborder = if is_focused {
                    theme.float_focus.to_f32x4()
                } else {
                    theme.float_accent.to_f32x4()
                };
                if let Some(backend) = state.backends.get(&float.pane.id.0) {
                    let data = backend.render_data();
                    render_backend_data(
                        &data,
                        fx,
                        fy,
                        fw,
                        fh,
                        &mut state.text_renderer,
                        &mut state.primitive_renderer,
                        theme,
                    );
                } else {
                    state.primitive_renderer.draw_rect(
                        fx,
                        fy,
                        fw,
                        fh,
                        theme.float_background.to_f32x4(),
                    );
                }
                state
                    .primitive_renderer
                    .draw_border(fx, fy, fw, fh, fborder, border_width * 2.0);
                // Draw floating pane name centered
                let float_name = &float.pane.title;
                let f_name_size = (fw.min(fh) * 0.25).clamp(24.0, 72.0);
                let f_name_color = if is_focused {
                    [1.0, 1.0, 1.0, 0.9]
                } else {
                    [1.0, 1.0, 1.0, 0.4]
                };
                let f_name_w = f_name_size * float_name.len() as f32 * 0.6;
                let f_name_x = fx + (fw - f_name_w) / 2.0;
                let f_name_y = fy + (fh - f_name_size) / 2.0;
                state.text_renderer.queue_text(
                    float_name,
                    f_name_x,
                    f_name_y,
                    f_name_size,
                    f_name_color,
                );
            }
        }

        // ── Detached pane (interactive move) ──
        mouse::render_detached_pane(state, (pane_area.x, pane_area.y, pane_area.w, pane_area.h));

        // ── Insert hint ──
        mouse::render_insert_hint(state, (pane_area.x, pane_area.y, pane_area.w, pane_area.h));

        // ── Pane select / swap letter overlay ──
        if let Some(candidates) = state.input_mode.candidates() {
            let letter_size = 48.0f32;
            let label_color = [1.0, 0.9, 0.3, 0.9];
            for (ch, target_id) in candidates {
                // Skip the focused pane — no need to swap with yourself.
                if Some(*target_id) == state.focused_pane {
                    continue;
                }
                let mut found = false;
                // Check scrolling panes in the ACTIVE workspace
                for (pane_id, rect) in &pane_positions {
                    if pane_id.0 == *target_id {
                        let px = pane_area.x + ws_offset.0 + rect.loc.x as f32;
                        let py = pane_area.y + ws_offset.1 + rect.loc.y as f32;
                        let pw = rect.size.w as f32;
                        let ph = rect.size.h as f32;
                        let lx = px + (pw - letter_size * 0.6) / 2.0;
                        let ly = py + (ph - letter_size) / 2.0;
                        let label = ch.to_string();
                        state
                            .text_renderer
                            .queue_text(&label, lx, ly, letter_size, label_color);
                        found = true;
                        break;
                    }
                }
                if !found {
                    // Check floating panes in the ACTIVE workspace
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
                                state.text_renderer.queue_text(
                                    &label,
                                    lx,
                                    ly,
                                    letter_size,
                                    label_color,
                                );
                                found = true;
                                break;
                            }
                        }
                    }
                }
                // Panes in non-active workspaces are not visible on screen, so
                // their letters are intentionally not rendered. The sidebar
                // still shows all candidate letters across workspaces.
                let _ = found;
            }
        }

        state
            .primitive_renderer
            .render(&state.device, &view, &mut encoder);
        state
            .text_renderer
            .render(&state.device, &state.queue, &view, &mut encoder);

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
            WindowEvent::Resized(phys) if phys.width > 0 && phys.height > 0 => {
                state.surface_config.width = phys.width;
                state.surface_config.height = phys.height;
                state
                    .surface
                    .configure(&state.device, &state.surface_config);
                let log_w = phys.width as f32 / state.scale_factor as f32;
                let log_h = phys.height as f32 / state.scale_factor as f32;
                state
                    .primitive_renderer
                    .set_screen_size(&state.queue, log_w, log_h);
                state
                    .text_renderer
                    .set_screen_size(&state.queue, log_w, log_h);
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
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }
                state.needs_redraw = true;

                let is_ctrl = state.modifiers.control_key();
                let _is_alt = state.modifiers.alt_key();
                let is_shift = state.modifiers.shift_key();

                let log_key = &event.logical_key;
                let key_text = log_key.to_text().unwrap_or("").to_string();

                // Build a KeyCombo from the current key event for comparison.
                let event_combo = keymap::KeyCombo {
                    key: normalize_key_text(
                        &event.logical_key,
                        &key_text,
                        is_shift,
                        is_ctrl,
                        &event.physical_key,
                    ),
                    ctrl: is_ctrl,
                    shift: is_shift,
                    alt: state.modifiers.alt_key(),
                    super_: state.modifiers.super_key(),
                };

                // Check if current key matches the configured prefix combo.
                let is_prefix = event_combo_matches(&event_combo, &state.prefix_combo)
                    // macOS special case: Ctrl+letter may report as control character
                    // (e.g. Ctrl+A → \u{1}, Ctrl+B → \u{2}). Match the configured prefix
                    // key against the control-char equivalent.
                    || (state.prefix_combo.ctrl
                        && state.prefix_combo.key.len() == 1
                        && state.prefix_combo.key.chars().next().unwrap().is_ascii_lowercase()
                        && {
                            let expected_ctrl = (state.prefix_combo.key.as_bytes()[0] - b'a' + 1) as char;
                            key_text == String::from(expected_ctrl)
                                || matches!(log_key, winit::keyboard::Key::Character(c) if c.starts_with(expected_ctrl))
                        });

                // Handle Rename mode separately (needs mutable buffer access)
                if let InputMode::Rename { target, buffer } = &mut state.input_mode {
                    let is_escape = matches!(
                        event.logical_key,
                        winit::keyboard::Key::Named(NamedKey::Escape)
                    );
                    let is_enter = matches!(
                        event.logical_key,
                        winit::keyboard::Key::Named(NamedKey::Enter)
                    );
                    let is_backspace = matches!(
                        event.logical_key,
                        winit::keyboard::Key::Named(NamedKey::Backspace)
                    );

                    if is_escape {
                        state.input_mode = InputMode::Normal;
                    } else if is_enter {
                        // Commit rename
                        let new_name = buffer.trim().to_string();
                        match target {
                            RenameTarget::Workspace(ws_idx) => {
                                if let Some(ws) = state.session.workspaces.get_mut(*ws_idx) {
                                    ws.name = if new_name.is_empty() {
                                        None
                                    } else {
                                        Some(new_name)
                                    };
                                }
                            }
                            RenameTarget::Column { ws_idx, col_idx } => {
                                if let Some(ws) = state.session.workspaces.get_mut(*ws_idx)
                                    && let Some(col) = ws.scrolling.columns.get_mut(*col_idx)
                                {
                                    col.name = if new_name.is_empty() {
                                        None
                                    } else {
                                        Some(new_name)
                                    };
                                }
                            }
                            RenameTarget::Pane(pane_id) => {
                                if let Some(ws) = state.session.active_workspace_mut()
                                    && let Some(pane) =
                                        ws.find_pane_mut(heca_core::layout::PaneId(*pane_id))
                                {
                                    pane.title = if new_name.is_empty() {
                                        format!("pane{}", pane_id)
                                    } else {
                                        new_name
                                    };
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

                // Handle ConfirmDelete mode — y/n/esc
                if let InputMode::ConfirmDelete { action, .. } = &state.input_mode {
                    let is_escape = matches!(
                        event.logical_key,
                        winit::keyboard::Key::Named(NamedKey::Escape)
                    );
                    let is_y = key_text == "y" || key_text == "Y";
                    let is_n = key_text == "n" || key_text == "N";

                    if is_escape || is_n {
                        state.input_mode = InputMode::Normal;
                    } else if is_y {
                        // Extract the action to execute (clone it out)
                        let action = action.as_ref().clone();
                        state.input_mode = InputMode::Normal;
                        self.registry.execute(&action, state);
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
                        let global_action = self.keymap.resolve("global", &event_combo).cloned();
                        if let Some(act) = global_action {
                            self.registry.execute(&act, state);
                            return;
                        }

                        // Forward key to focused pane's backend (terminal input)
                        if let Some(pane_id) = state.focused_pane
                            && let Some(backend) = state.backends.get_mut(&pane_id)
                        {
                            let input_bytes =
                                winit_key_to_terminal_input(&event.logical_key, &key_text, is_ctrl);
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
                                && let Some(backend) = state.backends.get_mut(&pane_id)
                            {
                                backend.process_input(&[0x02]);
                            }
                            return;
                        }

                        // Ignore bare modifier keys (Shift, etc.) in prefix mode.
                        // The user may hold Shift while pressing the action key; we should
                        // wait for the actual character key, not exit on Shift alone.
                        let is_modifier_only = key_text.is_empty()
                            && matches!(
                                event.logical_key,
                                winit::keyboard::Key::Named(
                                    winit::keyboard::NamedKey::Shift
                                        | winit::keyboard::NamedKey::Control
                                        | winit::keyboard::NamedKey::Alt
                                        | winit::keyboard::NamedKey::Super
                                        | winit::keyboard::NamedKey::Hyper
                                        | winit::keyboard::NamedKey::Meta
                                )
                            );
                        if is_modifier_only {
                            return;
                        }

                        // In prefix mode, pass the REAL modifier state. The user may intentionally
                        // press Ctrl+another key after the prefix (e.g. Ctrl+h for swap_left).
                        // The prefix key itself (Ctrl+B) is already handled above by is_prefix.
                        let combo = keymap::KeyCombo {
                            key: normalize_key_text(
                                &event.logical_key,
                                &key_text,
                                is_shift,
                                is_ctrl,
                                &event.physical_key,
                            ),
                            ctrl: is_ctrl,
                            shift: is_shift,
                            alt: false,
                            super_: false,
                        };

                        // Check mode triggers first (e.g. prefix+r → resize mode).
                        let mut entered_mode = None;
                        for (mode_name, (trigger_combo, sticky)) in &self.mode_triggers {
                            if event_combo_matches(&combo, trigger_combo) {
                                entered_mode = Some((mode_name.clone(), *sticky));
                                break;
                            }
                        }
                        if let Some((mode_name, _sticky)) = entered_mode {
                            state.input_mode = InputMode::Mode { name: mode_name };
                            state.prefix_entered_at = None;
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
                        let is_escape = matches!(
                            event.logical_key,
                            winit::keyboard::Key::Named(NamedKey::Escape)
                        );
                        if is_escape {
                            state.input_mode = InputMode::Normal;
                            state.needs_redraw = true;
                            return;
                        }

                        // Hardcoded chord: w → digit switches to workspace.
                        if sequence.len() == 1
                            && sequence[0].eq_ignore_ascii_case("w")
                            && let Some(digit) = key_text
                                .chars()
                                .next()
                                .filter(|c| c.is_ascii_digit())
                                .and_then(|c| c.to_digit(10))
                        {
                            let ws_idx = (digit as usize).saturating_sub(1);
                            if ws_idx < state.session.workspaces.len() {
                                self.registry
                                    .execute(&WmAction::FocusWorkspace { ws_idx }, state);
                                state.needs_redraw = true;
                            }
                            state.input_mode = InputMode::Normal;
                            return;
                        }

                        // Unknown chord key — cancel.
                        state.input_mode = InputMode::Normal;
                        state.needs_redraw = true;
                    }
                    InputMode::Mode { name } => {
                        let name = name.clone();
                        let is_escape = matches!(
                            event.logical_key,
                            winit::keyboard::Key::Named(NamedKey::Escape)
                        );
                        let is_enter = matches!(
                            event.logical_key,
                            winit::keyboard::Key::Named(NamedKey::Enter)
                        );
                        if is_escape || is_enter {
                            state.input_mode = InputMode::Normal;
                            state.needs_redraw = true;
                            return;
                        }
                        let combo = keymap::KeyCombo {
                            key: normalize_key_text(
                                &event.logical_key,
                                &key_text,
                                is_shift,
                                is_ctrl,
                                &event.physical_key,
                            ),
                            ctrl: is_ctrl,
                            shift: is_shift,
                            alt: false,
                            super_: false,
                        };
                        if let Some(mode_map) = self.mode_keymaps.get(&name)
                            && let Some(action) = mode_map.resolve(&name, &combo).cloned()
                        {
                            let sticky = self
                                .mode_triggers
                                .get(&name)
                                .map(|(_, s)| *s)
                                .unwrap_or(true);
                            self.registry.execute(&action, state);
                            if !sticky {
                                state.input_mode = InputMode::Normal;
                                state.needs_redraw = true;
                            }
                        }
                    }
                    InputMode::PaneSelect { candidates } => {
                        let candidates = candidates.clone();
                        state.input_mode = InputMode::Normal;
                        let typed = key_text
                            .chars()
                            .next()
                            .or_else(|| match event.physical_key {
                                winit::keyboard::PhysicalKey::Code(c) => {
                                    let s = format!("{:?}", c);
                                    s.strip_prefix("Key").and_then(|n| n.chars().next())
                                }
                                _ => None,
                            })
                            .map(|c| c.to_ascii_lowercase());
                        if let Some(ch) = typed
                            && let Some((_, target_id)) = candidates.iter().find(|(c, _)| *c == ch)
                        {
                            self.registry.execute(
                                &WmAction::FocusPane {
                                    pane_id: *target_id,
                                },
                                state,
                            );
                        }
                        state.input_mode = InputMode::Normal;
                    }
                    InputMode::PaneSwap {
                        candidates,
                        focus_after,
                    } => {
                        let candidates = candidates.clone();
                        let should_focus = *focus_after;
                        state.input_mode = InputMode::Normal;

                        let typed = key_text
                            .chars()
                            .next()
                            .or_else(|| match event.physical_key {
                                winit::keyboard::PhysicalKey::Code(c) => {
                                    let s = format!("{:?}", c);
                                    s.strip_prefix("Key").and_then(|n| n.chars().next())
                                }
                                _ => None,
                            })
                            .map(|c| c.to_ascii_lowercase());
                        let current_id = state.focused_pane;
                        if let Some(ch) = typed
                            && let Some(current_id) = current_id
                            && let Some((_, target_id)) = candidates.iter().find(|(c, _)| *c == ch)
                            && let Some((_, _, _)) = find_pane_location(&state.session, current_id)
                            && let Some((_, _, _)) = find_pane_location(&state.session, *target_id)
                        {
                            // Both panes are present in scrolling columns — dispatch Swap.
                            self.registry.execute(
                                &WmAction::Swap {
                                    a_id: current_id,
                                    b_id: *target_id,
                                },
                                state,
                            );

                            // Apply focus semantics.
                            if should_focus {
                                self.registry.execute(
                                    &WmAction::FocusPane {
                                        pane_id: current_id,
                                    },
                                    state,
                                );
                            } else {
                                self.registry.execute(
                                    &WmAction::FocusPane {
                                        pane_id: *target_id,
                                    },
                                    state,
                                );
                            }
                        }
                        state.needs_redraw = true;
                    }
                    InputMode::PaneTake {
                        candidates,
                        focus_after,
                    } => {
                        let candidates = candidates.clone();
                        let should_focus = *focus_after;
                        state.input_mode = InputMode::Normal;

                        let typed = key_text
                            .chars()
                            .next()
                            .or_else(|| match event.physical_key {
                                winit::keyboard::PhysicalKey::Code(c) => {
                                    let s = format!("{:?}", c);
                                    s.strip_prefix("Key").and_then(|n| n.chars().next())
                                }
                                _ => None,
                            })
                            .map(|c| c.to_ascii_lowercase());
                        if let Some(ch) = typed
                            && let Some((_, target_id)) = candidates.iter().find(|(c, _)| *c == ch)
                        {
                            self.registry.execute(
                                &WmAction::TakePane {
                                    pane_id: *target_id,
                                    focus_after: should_focus,
                                },
                                state,
                            );
                        }
                        state.needs_redraw = true;
                    }
                    InputMode::SidebarNav => {
                        let is_escape = matches!(
                            event.logical_key,
                            winit::keyboard::Key::Named(NamedKey::Escape)
                        );
                        let is_enter = matches!(
                            event.logical_key,
                            winit::keyboard::Key::Named(NamedKey::Enter)
                        );

                        if is_escape {
                            state.input_mode = InputMode::Normal;
                            state.needs_redraw = true;
                        } else if is_enter {
                            // Activate current sidebar item and exit sidebar mode
                            let item = state.sidebar_tree.current_item().cloned();
                            match &item {
                                Some(sidebar::SidebarItem::Pane { pane_id }) => {
                                    let target_pane_id = heca_core::layout::PaneId(*pane_id);
                                    let target_ws = state
                                        .session
                                        .workspaces
                                        .iter()
                                        .position(|ws| ws.find_pane(target_pane_id).is_some());
                                    if let Some(ws_idx) = target_ws {
                                        if ws_idx != state.session.active_workspace_idx {
                                            self.registry.execute(
                                                &WmAction::FocusWorkspace { ws_idx },
                                                state,
                                            );
                                        }
                                        self.registry.execute(
                                            &WmAction::FocusPane { pane_id: *pane_id },
                                            state,
                                        );
                                    }
                                }
                                Some(sidebar::SidebarItem::Workspace { .. }) => {
                                    let ws_idx = state
                                        .sidebar_tree
                                        .cursor_workspace_index()
                                        .unwrap_or(state.session.active_workspace_idx);
                                    if ws_idx != state.session.active_workspace_idx {
                                        self.registry
                                            .execute(&WmAction::FocusWorkspace { ws_idx }, state);
                                    }
                                }
                                _ => {}
                            }
                            state.input_mode = InputMode::Normal;
                            state.needs_redraw = true;
                        } else {
                            let combo = keymap::KeyCombo {
                                key: normalize_key_text(
                                    &event.logical_key,
                                    &key_text,
                                    is_shift,
                                    is_ctrl,
                                    &event.physical_key,
                                ),
                                ctrl: is_ctrl,
                                shift: is_shift,
                                alt: false,
                                super_: false,
                            };
                            let action = self.keymap.resolve("sidebar", &combo).cloned();
                            if let Some(act) = action {
                                self.registry.execute(&act, state);
                            }
                        }
                    }
                    // Rename, ConfirmDelete, and PaneTake are handled by early
                    // return before this match — use a wildcard for the rest.
                    _ => {}
                }
            }
            WindowEvent::ModifiersChanged(new_mods) => {
                state.modifiers = new_mods.state();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let pos = (
                    position.x as f32 / state.scale_factor as f32,
                    position.y as f32 / state.scale_factor as f32,
                );
                state.mouse.pos = pos;
                if let Some(action) = mouse::on_cursor_moved(state, pos) {
                    self.registry.execute(&action, state);
                }
                state.needs_redraw = true;
            }
            WindowEvent::MouseInput {
                state: button_state,
                button,
                ..
            } => {
                if let Some(action) = mouse::on_mouse_input(state, button, button_state) {
                    self.registry.execute(&action, state);
                }
                state.needs_redraw = true;
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Config reload requested via keybinding — do it before borrowing state.
        let needs_reload = self
            .state
            .as_ref()
            .map(|s| s.pending_reload)
            .unwrap_or(false);
        if needs_reload {
            if let Some(state) = self.state.as_mut() {
                state.pending_reload = false;
            }
            self.reload_config();
        }

        if let Some(ref mut state) = self.state {
            // Prefix / Chord mode auto-timeout: exit if inactive > 500 ms.
            let should_timeout = matches!(
                state.input_mode,
                InputMode::Prefix | InputMode::Chord { .. }
            ) && state
                .prefix_entered_at
                .is_some_and(|entered| entered.elapsed() >= Duration::from_millis(500));
            if should_timeout {
                state.input_mode = InputMode::Normal;
                state.prefix_entered_at = None;
                state.needs_redraw = true;
            }

            // Edge scroll during drag.
            let edge_scrolled = mouse::process_edge_scroll(state);
            if edge_scrolled {
                state.needs_redraw = true;
            }

            // Keep frame loop running while drag is active (for smooth visual feedback).
            if !matches!(state.mouse.drag_state, DragState::None) {
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

            let needs_frame =
                state.needs_redraw || backend_has_data || state.session.are_animations_ongoing();
            if needs_frame {
                state.window.request_redraw();
            }

            // Use WaitUntil during animations (60fps cap), Wait when idle (0% CPU).
            if state.session.are_animations_ongoing() {
                event_loop.set_control_flow(ControlFlow::WaitUntil(
                    Instant::now() + Duration::from_millis(16),
                ));
            } else {
                event_loop.set_control_flow(ControlFlow::Wait);
            }
        }
    }
}

/// Edge scroll: auto-scroll the layout when the pointer is near the left/right
/// edge of the content area. Returns true if scrolling is active.
fn main() {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = HecaApp::new();
    event_loop
        .run_app(&mut app)
        .expect("Failed to run event loop");
}
