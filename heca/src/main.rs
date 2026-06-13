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

use app::events::handle_window_event;
use app::events::AppEvent;
pub(crate) use app::focus::switch_workspace_tracked;
use app::lifecycle::{handle_about_to_wait, poll_backends};
use app::terminal_metrics::refresh_terminal_cell_size;
use heca_core::layout::PaneId;
pub(crate) use app::mutations::{
    destroy_empty_workspace, move_column_to_workspace, move_pane_to_column,
    move_pane_to_workspace_column,
};
use app::registry::{build_keymap, build_modes, build_registry};
pub(crate) use app::render::update_session_viewport;
pub(crate) use app::selection::{collect_all_pane_candidates, find_pane_location};
use app::startup::init_state as build_initial_state;
use app_state::AppState;
use heca_config::theme::AppConfig;
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
    #[allow(dead_code)]
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
            let new_config = AppConfig::load();
            self.app_config = new_config.clone();
            self.keymap = build_keymap(&self.app_config.config);
            let (new_mode_keymaps, new_mode_triggers) = build_modes(&self.app_config.config);
            self.mode_keymaps = new_mode_keymaps;
            self.mode_triggers = new_mode_triggers;
            state.theme = self.app_config.theme.clone();
            state
                .text_renderer
                .set_font_family(&self.app_config.theme.font_family);
            refresh_terminal_cell_size(state);
            state.prefix_combo = keymap::KeyCombo::parse(&self.app_config.config.keys.prefix);
            state.mouse_enabled = self.app_config.config.settings.mouse;
            state.auto_scroll_edge = self.app_config.config.settings.auto_scroll_edge;
            state.interactive_move_modifier =
                self.app_config.config.settings.interactive_move_modifier;
            state.needs_redraw = true;
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
                if backend_poll.has_data || backend_poll.closed_any {
                    state.needs_redraw = true;
                    state.window.request_redraw();
                }
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
