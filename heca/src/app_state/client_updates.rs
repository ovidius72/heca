//! Apply received content before reacting to its changes. The window keeps its own view.

use super::{AppState, layout_door::Asked};

impl AppState {
    pub(crate) fn receive_client(&mut self, update: crate::client::Update) -> Vec<(u64, String)> {
        let before = self.session.shape();
        if update.session_attached {
            self.chrome_state.panes.clear_feedback();
            // Attached replaces backend/view identities even when a restart retains SessionId.
            self.terminals.clear();
            self.terminal_layers.clear();
            self.terminal_zoom = Default::default();
            self.chrome_tree = None;
            if let Some(overlay) = self.session_dialog.take() {
                let emit = crate::chrome::ChromeIntentEmitter::new(&self.event_proxy, crate::app::interaction::InteractionSource::Keyboard);
                emit.fire(crate::app::interaction::InteractionIntent::ActivateAction(crate::input::WmAction::CloseOverlay { overlay: Some(overlay) }));
            }
            if self.session.id != self.client.session.id {
                self.pending_reload = Some(super::ReloadReason::Session);
                crate::chrome::discard_overlays(self);
                self.view = heca_core::layout::WindowView::new(self.layout().viewport(), self.scale_factor);
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
            self.session = content.rebuild(self.client.session.id, self.session.options.clone());
        }
        if let Some(views) = update.notifications {
            self.notifications = views;
            self.rebuild_toasts();
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
        if update.session_deleted { crate::handlers::handle_detach_session(self, &crate::input::WmAction::DetachSession); }
        else if update.session_terminated || (self.session_dialog.is_none()
            && self.client.session.status == crate::server::sessions::SessionStatus::Terminated) {
            crate::handlers::session_unavailable(self, false);
        }
        let mut intents = Vec::new();
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
