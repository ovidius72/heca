//! Apply received content before reacting to its changes. The window keeps its own view.

use super::{AppState, layout_door::Asked};

impl AppState {
    pub(crate) fn receive_client(&mut self, update: crate::client::Update) -> Vec<(u64, String)> {
        let before = self.session.shape();
        if update.session_attached {
            self.chrome_state.panes.clear_feedback();
            // Attached replaces backend/view identities even when a restart retains SessionId.
            self.terminals.clear();
            crate::chrome::clear_workspace(self);
            self.terminal_layers.clear();
            self.terminal_zoom = Default::default();
            self.chrome_tree = None;
            if let Some(overlay) = self.session_dialog.take() {
                let emit = crate::chrome::ChromeIntentEmitter::new(&self.event_proxy, crate::app::interaction::InteractionSource::Keyboard);
                overlay.closer(&emit)();
            }
            if self.session.id != self.client.session.id {
                self.pending_reload = Some(super::ReloadReason::Session);
                crate::chrome::discard_overlays(self);
                self.focused_pane = None;
                self.last_focused = None;
                self.last_visited_ws_idx = None;
                self.last_visited_pane_per_ws.clear();
                self.expose_cursor_per_ws.clear();
                self.expose_cursor_ws = None;
                self.selection = super::SelectionState::new();
            }
            self.notifications.clear();
            self.rebuild_toasts();
        }
        if let Some(content) = update.content {
            self.client.prune(&content);
            let session = content.rebuild(self.client.session.id, self.session.options.clone());
            if update.session_attached {
                self.view.replace_session(&mut self.session, session, self.scale_factor);
            } else { self.session = session; }
        }
        self.view.retain_sessions(|id| self.client.sessions.iter().any(|session| session.id == id));
        if let Some(views) = update.notifications {
            self.notifications = views;
            self.rebuild_toasts();
            self.needs_redraw = true;
        }
        // A session change, or leaving one, resets what services show; then what they sent applies.
        let left = update.session_attached || update.session_deleted || update.session_detached;
        if left || !update.services.is_empty() {
            crate::entry::service::client::apply_updates(left, update.services, |service| self.client.resync_service(service));
            self.needs_redraw = true;
        }
        if let Some(windows) = update.windows {
            // If the window we follow has left the session, resume our own view.
            if let Some(target) = self.client.following()
                && !windows.iter().any(|window| window.id == target)
            {
                self.client.follow(None);
            }
            self.windows = windows;
        }
        if let Some(snapshot) = update.follow_view {
            // Mirror the followed window's view onto ours, keeping our own size and scale.
            self.view.mirror(&snapshot);
            self.layout_mut().reconcile_floats();
            self.needs_redraw = true;
        }
        self.apply(update.changes);
        for answer in update.answers {
            self.apply_answer(answer.changes, Some(Asked { action: &answer.action, name: &answer.name, before: &answer.before }));
        }
        for (pane, code) in update.exits {
            self.chrome_state.events().emit(crate::chrome::ChromeEvent::PaneExited { pane, code });
        }
        for text in update.effects.clipboard_writes { crate::handlers::set_system_clipboard(&text); }
        for effect in update.reads {
            match effect {
                crate::client::ReadEffect::Clipboard { text, selection } => crate::handlers::finish_copy(self, selection, text),
                crate::client::ReadEffect::Matches { terminal, matches } =>
                    crate::app::terminal_host::receive_matches(self, terminal, matches),
            }
        }
        for error in update.errors { self.status_note = Some(error); }
        let mut intents = Vec::new();
        if update.session_deleted || update.session_detached {
            crate::chrome::discard_overlays(self);
            self.notifications.clear();
            self.rebuild_toasts();
            crate::handlers::session_choice(self, if update.session_deleted {
                crate::handlers::SessionChoice::Deleted
            } else { crate::handlers::SessionChoice::Detached });
        }
        else if update.session_terminated || (self.session_dialog.is_none()
            && !self.client.detached
            && self.client.session.status == crate::server::sessions::SessionStatus::Terminated) {
            crate::handlers::session_unavailable(self, false);
        }
        for mut intent in update.intents {
            match self.client.normalize_server_intent(&mut intent) {
                Ok(()) => intents.push(intent),
                Err(error) => self.status_note = Some(error.to_string()),
            }
        }
        crate::providers::emit_queued(self, intents);
        if update.session_attached || self.session.shape() != before { crate::app::mutations::after_layout_change(self); }
        self.needs_redraw |= update.effects.has_data || update.effects.bell_any;
        if update.effects.bell_any { crate::app::lifecycle::apply_bell_policy(self); }
        update.rpc
    }
}
