mod actions;
mod app;
mod app_state;
mod chrome;
mod handlers;
mod host;
mod input;
mod keymap;
mod mouse;
mod rpc;
mod shortcut;
mod sidebar;

use app::events::AppEvent;
use app::events::handle_window_event;
pub(crate) use app::focus::switch_workspace_tracked;
use app::interaction::dispatch_intent;
use app::lifecycle::{handle_about_to_wait, poll_backends};
pub(crate) use app::mutations::{
    destroy_empty_workspace, move_column_to_workspace, move_pane_to_column,
    move_pane_to_workspace_column,
};
use app::registry::{build_keymap, build_modes, build_registry};
pub(crate) use app::render::update_session_viewport;
pub(crate) use app::selection::{collect_all_pane_candidates, find_pane_location};
use app::startup::init_state as build_initial_state;
use app::terminal_metrics::refresh_terminal_cell_size;
use app_state::AppState;
use heca_config::theme::AppConfig;
use heca_core::layout::PaneId;
use input::WmAction;
use std::collections::HashMap;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};

use winit::window::WindowId;

/// Distinct pane names so you can visually identify what's moving.
const PANE_NAMES: &[&str] = &[
    "Red", "Green", "Blue", "Yellow", "Cyan", "Magenta", "Orange", "Purple", "Lime", "Pink",
    "Teal", "Coral",
];

pub(crate) fn pane_name(id: PaneId) -> String {
    PANE_NAMES
        .get((id.0 as usize).saturating_sub(1) % PANE_NAMES.len())
        .unwrap_or(&"?")
        .to_string()
}

struct HecaApp {
    state: Option<Box<AppState>>,
    app_config: AppConfig,
    event_proxy: EventLoopProxy<AppEvent>,
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
    /// Returns the parsed action on success, or an error on failure.
    // Transitional: will be used by the RPC server / socket listener in Phase 5.
    #[expect(dead_code, reason = "Reserved for the Phase 5 RPC server path.")]
    pub fn execute_rpc_command(&mut self, cmd: &str) -> Result<WmAction, rpc::RpcError> {
        let state = self.state.as_mut().ok_or(rpc::RpcError::NotInitialized)?;
        let action = rpc::parse_rpc_command(cmd)?;
        self.registry.execute(&action, state);
        Ok(action)
    }

    fn new(event_proxy: EventLoopProxy<AppEvent>) -> Self {
        let app_config = AppConfig::load();
        let registry = build_registry();
        let keymap = build_keymap(&app_config.config);
        let (mode_keymaps, mode_triggers) = build_modes(&app_config.config);

        Self {
            state: None,
            app_config,
            event_proxy,
            registry,
            keymap,
            mode_keymaps,
            mode_triggers,
        }
    }

    fn reload_config(&mut self) {
        if let Some(ref mut state) = self.state {
            // Try to load the config file. On error, keep the current working
            // config and report the problem — a bad config must not silently
            // overwrite the user's working settings.
            let new_config = match heca_config::loader::AppConfig::try_load() {
                Ok(cfg) => cfg,
                Err(e) => {
                    eprintln!("[heca] reload failed: {e}");
                    eprintln!("[heca] fix config.toml and press prefix+Shift+r to retry");
                    return;
                }
            };
            self.app_config = new_config;
            self.keymap = build_keymap(&self.app_config.config);
            let (new_mode_keymaps, new_mode_triggers) = build_modes(&self.app_config.config);
            self.mode_keymaps = new_mode_keymaps;
            self.mode_triggers = new_mode_triggers;
            state.theme = self.app_config.theme.clone();
            state.programs = self.app_config.config.programs.clone();
            // Appearance: opacity re-reads every frame, so updating the snapshot
            // makes `transparency` (the amount) live-reload. The OS vibrancy
            // material is applied once at startup and NOT re-applied here — doing
            // so would stack another container/effect view each reload. Changing
            // `vibrancy` (or toggling transparency on↔off) needs a restart.
            state.appearance = self.app_config.config.appearance.clone();
            // Font config (families + sizes) is decoupled from the color theme;
            // reload it so `prefix+Shift+r` picks up `[font]` changes live.
            state.font_config = self.app_config.config.font.clone();
            // Sidebar width is config-driven; re-apply on reload (resets any runtime
            // drag-resize to the configured/clamped value).
            let sidebar_width = self.app_config.config.appearance.effective_sidebar_width();
            state.chrome_state.set_left_size(sidebar_width);
            state.chrome_state.set_right_size(sidebar_width);
            state
                .text_renderer
                .set_font_family(self.app_config.config.font.family.ui_normal());
            refresh_terminal_cell_size(state);
            state.terminal_layers.clear();
            state.prefix_combo = keymap::KeyCombo::parse(&self.app_config.config.keys.prefix);
            state.pane_action_hints = crate::chrome::PaneActionHints::from_keys(
                &self.app_config.config.keys,
                &state.prefix_combo,
            );
            state.mouse_enabled = self.app_config.config.settings.mouse;
            state.auto_scroll_edge = self.app_config.config.settings.auto_scroll_edge;
            state.shell_integration_enabled = self.app_config.config.settings.shell_integration;
            state.terminal_scrollback_lines =
                self.app_config.config.settings.terminal_scrollback_lines;
            state.interactive_move_modifier =
                self.app_config.config.settings.interactive_move_modifier;
            // Pane gap and chrome geometry changes must reflow the real viewport
            // path so cached column widths, pane sizes, and working areas stay
            // coherent after reload.
            let pane_gap = self
                .app_config
                .config
                .appearance
                .effective_pane_gap(&self.app_config.theme) as f64;
            if (state.session.options.gaps - pane_gap).abs() > f64::EPSILON {
                state.session.options.gaps = pane_gap;
                for ws in &mut state.session.workspaces {
                    ws.scrolling.options.gaps = pane_gap;
                }
            }
            update_session_viewport(state);
            // Force a full chrome rebuild so STRUCTURAL config (border style, pane
            // info bar, etc.) re-applies — the retained tree is otherwise only
            // rebuilt when `chrome_signature` changes, which can miss config edits.
            state.chrome_tree = None;
            state.needs_redraw = true;
            // Reload runs in `about_to_wait`; request an explicit redraw so the
            // reloaded config takes effect immediately instead of on the next input.
            state.window.request_redraw();
        }
    }

    async fn init_state(&mut self, event_loop: &ActiveEventLoop) -> Box<AppState> {
        build_initial_state(&self.app_config, event_loop, self.event_proxy.clone()).await
    }
}

impl ApplicationHandler<AppEvent> for HecaApp {
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

        handle_window_event(
            event_loop,
            &self.registry,
            &self.keymap,
            &self.mode_keymaps,
            &self.mode_triggers,
            state,
            event,
        );
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
            handle_about_to_wait(event_loop, state);
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AppEvent) {
        let Some(state) = self.state.as_mut() else {
            return;
        };

        match event {
            AppEvent::BackendWake => {
                let backend_poll = poll_backends(state);
                let chrome_runtime_changed = crate::chrome::sync_chrome_state(state);
                if backend_poll.has_data || backend_poll.closed_any || chrome_runtime_changed {
                    state.mark_full_redraw();
                    state.window.request_redraw();
                }
            }
            AppEvent::RequestRedraw => {
                state.needs_redraw = true;
                state.window.request_redraw();
            }
            AppEvent::ChromeIntent { source, intent } => {
                dispatch_intent(state, &self.registry, source, intent);
                state.mark_full_redraw();
                state.window.request_redraw();
            }
        }
    }
}

/// Edge scroll: auto-scroll the layout when the pointer is near the left/right
/// edge of the content area. Returns true if scrolling is active.
fn main() {
    let event_loop = EventLoop::<AppEvent>::with_user_event()
        .build()
        .expect("Failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let event_proxy = event_loop.create_proxy();
    let mut app = HecaApp::new(event_proxy);
    event_loop
        .run_app(&mut app)
        .expect("Failed to run event loop");
}
