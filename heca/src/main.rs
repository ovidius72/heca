mod actions;
mod app;
mod app_state;
mod chrome;
mod components;
mod handlers;
mod host;
mod input;
mod keymap;
mod mouse;
mod providers;
mod rpc;
mod search_state;
mod shortcut;

use app::events::AppEvent;
use app::events::handle_window_event;
pub(crate) use app::focus::switch_workspace_tracked;
use app::interaction::dispatch_intent;
use app::lifecycle::{handle_about_to_wait, poll_backends};
pub(crate) use app::mutations::{
    destroy_empty_workspace, move_column_to_workspace, move_pane_to_column,
    move_pane_to_workspace_column,
};
use app::registry::{build_keymaps, build_registry};
pub(crate) use app::render::update_session_viewport;
pub(crate) use app::selection::{collect_all_pane_candidates, find_pane_location};
use app::startup::init_state as build_initial_state;
use app::backend_factory::terminal_palette_defaults;
use app::terminal_metrics::refresh_terminal_cell_size;
use app_state::AppState;
use heca_config::theme::AppConfig;
use heca_core::layout::PaneId;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};

use winit::window::WindowId;

/// A newly spawned pane has **no name** until the user renames it — its display falls back to
/// the running program (a separate chrome segment), never a placeholder. Kept as a function (not
/// an inline `String::new()`) because it is threaded as a `pane_name_fn` pointer into the layout
/// swap/placeholder path.
pub(crate) fn pane_name(_id: PaneId) -> String {
    String::new()
}

struct HecaApp {
    state: Option<Box<AppState>>,
    app_config: AppConfig,
    event_proxy: EventLoopProxy<AppEvent>,
    registry: actions::ActionRegistry,
    /// Every resolved keymap layer — flat, per mode, per component kind, plus the mode triggers.
    /// One artefact, one lifetime: built from config at load and rebuilt together on reload.
    keymaps: keymap::Keymaps,
    /// What collided while assembling the app, reported once at startup.
    conflicts: crate::app::conflicts::Conflicts,
}

impl HecaApp {
    /// Parse and execute an RPC command string.
    ///
    /// **Both vocabularies, one gate** (F003/P086/T372): a named built-in command and the generic
    /// `action <name> [key=value …]` verb both go out as an `Intent` through the same
    /// `dispatch_view_intent` a click, a key and a menu entry use. So a component's or plugin's
    /// declared action is callable at last — and everything a script asks for is judged by the same
    /// policy in the same domain, instead of the old direct `registry.execute`, which was the one
    /// door that skipped the router.
    ///
    /// The reply says what became of it, because a script cannot be told "ok" when nothing ran: an
    /// unknown name, an action the current domain refuses, and one whose component is not mounted
    /// are three different answers.
    // Transitional: will be used by the RPC server / socket listener in Phase 5.
    #[expect(dead_code, reason = "Reserved for the Phase 5 RPC server path.")]
    pub fn execute_rpc_command(&mut self, cmd: &str) -> Result<(), rpc::RpcError> {
        use crate::app::interaction::{
            dispatch_action, dispatch_view_intent, InteractionSource, IntentOutcome,
        };
        let state = self.state.as_mut().ok_or(rpc::RpcError::NotInitialized)?;
        match rpc::parse_rpc(cmd)? {
            // A built-in keeps its own spelling and its `WmAction`; what changes is that it is now
            // **routed** rather than executed directly.
            rpc::RpcCommand::Builtin(action) => {
                dispatch_action(state, &self.registry, InteractionSource::Rpc, &action);
                Ok(())
            }
            rpc::RpcCommand::Intent { intent, dock } => {
                // **Which placement, and does it need the keyboard?** (F003/P085/T358)
                //
                // `--dock` names the seating outright. Without it, an action a *component* declared
                // still has to land somewhere, and `owning_mount` is the one rule that says where —
                // the same answer a click inside the component and a palette entry get.
                //
                // Either way the call goes through focus-then-act, so a component's
                // `ContainerFocused` verb ("act on the row my cursor is on") is reachable from a
                // script without the script first faking a keypress: the container is focused, as
                // the user would have done, and the action is then judged by the ordinary policy.
                // A built-in has no owner and is dispatched straight, exactly as before.
                let container = dock.or_else(|| {
                    state
                        .action_catalog
                        .find(&intent.action)
                        .and_then(|m| m.owner.as_ref())
                        .and_then(|_| crate::providers::owning_mount(state, &intent.action))
                });
                let outcome = match container {
                    Some(container) => crate::app::interaction::focus_container_then_action(
                        state,
                        &self.registry,
                        InteractionSource::Rpc,
                        &container,
                        &intent,
                    ),
                    None => {
                        dispatch_view_intent(state, &self.registry, InteractionSource::Rpc, &intent)
                    }
                };
                match outcome {
                    IntentOutcome::Ran => Ok(()),
                    IntentOutcome::Unknown => Err(rpc::RpcError::UnknownCommand(intent.action)),
                    IntentOutcome::Blocked => Err(rpc::RpcError::Blocked(intent.action)),
                    IntentOutcome::NotRunnable => Err(rpc::RpcError::NotRunnable(intent.action)),
                    IntentOutcome::MissingArgs => Err(rpc::RpcError::MissingArgs(intent.action)),
                }
            }
        }
    }

    /// Answer an action-metadata **introspection** query (`list-actions` / `describe-action <name>`)
    /// against the runtime catalog, returning JSON. `None` if `cmd` is not an introspection command
    /// — the RPC server then routes it to [`execute_rpc_command`] to run as an action. A query
    /// returns data, never a [`WmAction`], which is why it is a separate entry point.
    // Transitional: consumed by the Phase 5 RPC server alongside `execute_rpc_command`.
    #[expect(dead_code, reason = "Reserved for the Phase 5 RPC server path.")]
    pub fn query_rpc_command(&self, cmd: &str) -> Option<Result<String, rpc::RpcError>> {
        let state = self.state.as_ref()?;
        rpc::introspect(&state.action_catalog, cmd)
    }

    fn new(event_proxy: EventLoopProxy<AppEvent>) -> Self {
        let app_config = AppConfig::load();
        let registry = build_registry();
        // Every collision — a key bound twice, an id declared twice — is collected here and
        // reported **once**, after the components have registered too (see `resumed`).
        let mut conflicts = crate::app::conflicts::Conflicts::default();
        let keymaps = build_keymaps(&app_config.config, &mut conflicts);

        Self {
            state: None,
            app_config,
            event_proxy,
            registry,
            keymaps,
            conflicts,
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
            // A reload re-reads the **file**, so the key collisions it found are stale and are
            // re-answered from scratch. The action-id collisions are not: components do not
            // re-register on reload, so those are still exactly as true as they were at startup and
            // would otherwise vanish from the report the moment the user pressed reload.
            let mut conflicts = crate::app::conflicts::Conflicts {
                actions: std::mem::take(&mut self.conflicts.actions),
                ..Default::default()
            };
            self.keymaps = build_keymaps(&self.app_config.config, &mut conflicts);
            self.conflicts = conflicts;
            // The layers were just rebuilt from a file that knows nothing about a plugin mounted
            // afterwards, so a reload would otherwise silently unbind every key it registered.
            crate::providers::bind_provider_keybindings(
                state,
                &mut self.keymaps.components,
                &mut self.keymaps.by_action,
            );
            // After the plugins' keys are back, for the same reason the startup report waits for
            // them: a report taken before everything has bound is not a report.
            self.conflicts.report();
            state.theme = self.app_config.theme.clone();
            state.programs = std::rc::Rc::new(self.app_config.config.programs.clone());
            // Appearance: opacity re-reads every frame, so updating the snapshot
            // makes `transparency` (the amount) live-reload. The OS vibrancy
            // material is applied once at startup and NOT re-applied here — doing
            // so would stack another container/effect view each reload. Changing
            // `vibrancy` (or toggling transparency on↔off) needs a restart.
            state.appearance = self.app_config.config.appearance.clone();
            // Re-read on reload like the rest: change the setting, press reload, the next palette
            // opens at the new size.
            state.command_palette_size = self.app_config.config.settings.command_palette_size;
            state.search_case = self.app_config.config.settings.search_case;
            state.search_history = self.app_config.config.settings.search_history;
            // **The layout's own options too.** They were read once at startup and never again, so
            // editing `overview_zoom`, `overview_gap`, `overview_zoom_from`, the pane gap or
            // `center_focused_column` and pressing reload appeared to do nothing — the settings
            // were live in the file and dead in the app (Antonio, 2026-08-11). One mapping,
            // `startup::layout_options_from`, shared by both paths so they cannot drift.
            state.session.options = crate::app::startup::layout_options_from(&self.app_config);
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
            // Retained pane headers bake theme colors / fonts into their tree at
            // build time, and `pane_header_key` intentionally has no theme identity
            // (themes only change on reload). Drop every header so `sync_pane_headers`
            // rebuilds them against the new theme next frame. Mirrors
            // `terminal_layers.clear()` and `chrome_tree = None`; without it, existing
            // panes keep stale (faint) icon colors after a theme swap while
            // freshly-created panes look correct.
            crate::chrome::clear_pane_headers(state);
            // The pane shells bake the theme too, so they are invalidated with the headers.
            crate::chrome::clear_panes(state);
            state.prefix_combo = keymap::KeyCombo::parse(&self.app_config.config.keys.prefix);
            state.widget_keymap = crate::app::registry::build_widget_keymap(&self.app_config.config);
            state.action_shortcuts = crate::chrome::ActionShortcuts::from_index(
                &self.keymaps.by_action,
                crate::shortcut::KeyStyle::Compact,
            );
            state.mouse_enabled = self.app_config.config.settings.mouse;
            state.auto_scroll_edge = self.app_config.config.settings.auto_scroll_edge;
            state.shell_integration_enabled = self.app_config.config.settings.shell_integration;
            state.pane_renamed_add_process_name =
                self.app_config.config.settings.pane_renamed_add_process_name;
            state.pane_show_cwd = self.app_config.config.settings.pane_show_cwd;
            state.terminal_scrollback_lines =
                self.app_config.config.settings.terminal_scrollback_lines;
            state.terminal_mouse_enabled = self.app_config.config.settings.terminal_mouse;
            state.terminal_wheel_scroll_lines =
                self.app_config.config.settings.terminal_wheel_scroll_lines;
            state.terminal_font_zoom_step =
                self.app_config.config.settings.terminal_font_zoom_step;
            state.mouse_wheel_change_font_size =
                self.app_config.config.settings.mouse_wheel_change_font_size;
            state.terminal_scroll_animations_enabled =
                self.app_config.config.settings.terminal_scroll_animations;
            state.show_left_sidebar = self.app_config.config.settings.show_left_sidebar;
            state.show_right_sidebar = self.app_config.config.settings.show_right_sidebar;
            state.show_top_bar = self.app_config.config.settings.show_top_bar;
            state.show_bottom_bar = self.app_config.config.settings.show_bottom_bar;
            state.confirm = self.app_config.config.confirm.clone();
            let link_detection = self.app_config.config.appearance.terminal.link_detection;
            let palette_defaults = terminal_palette_defaults(&state.theme);
            for backend in state.backends.values_mut() {
                backend.set_scroll_animations_enabled(state.terminal_scroll_animations_enabled);
                backend.set_link_detection(link_detection);
                backend.reload_terminal_config(
                    palette_defaults,
                    state.terminal_scrollback_lines,
                );
            }
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
            let mut state = pollster::block_on(self.init_state(event_loop));
            // Every mounted component's **declared** actions join the one catalog and registry here
            // — after the host has its providers, before any input can reach them. This is the host
            // half of the declaration: a component never mutates app state, so it cannot register
            // its own (F003/P085/T353).
            crate::providers::register_provider_actions(
                &mut state,
                &mut self.registry,
                &mut self.conflicts,
            );
            // Then the keys a plugin registered for them. Core components have theirs from the
            // config file already; this is the path for anything that has no entry in it.
            crate::providers::bind_provider_keybindings(
                &state,
                &mut self.keymaps.components,
                &mut self.keymaps.by_action,
            );
            // Only now does every layer exist, so this is the first moment a tooltip can be told
            // the truth about which key runs an action.
            state.action_shortcuts = crate::chrome::ActionShortcuts::from_index(
                &self.keymaps.by_action,
                crate::shortcut::KeyStyle::Compact,
            );
            // Everything that can register has now registered — config, built-ins and every
            // mounted component — so this is the first moment the report can be complete.
            self.conflicts.report();
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
            &self.keymaps,
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
    // `--keys-show` answers "what key runs this action" and exits — before the window, so a script
    // can ask without a GPU (F003/P086/T366).
    let args: Vec<String> = std::env::args().skip(1).collect();
    if crate::app::keys_show::run_if_requested(&args) {
        return;
    }
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
