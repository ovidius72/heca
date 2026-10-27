//! Apply received content before reacting to its changes. The window keeps its own view.

use super::{AppState, layout_door::Asked};

impl AppState {
    pub(crate) fn receive_client(&mut self, update: crate::client::Update) -> Vec<(u64, String)> {
        let before = self.session.shape();
        if let Some(content) = update.content {
            self.client.prune(&content);
            self.session = content.rebuild(self.session.id, self.session.options.clone());
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
        let mut intents = Vec::new();
        for mut intent in update.intents {
            match self.client.normalize_server_intent(&mut intent) {
                Ok(()) => intents.push(intent),
                Err(error) => self.status_note = Some(error.to_string()),
            }
        }
        crate::providers::emit_queued(self, intents);
        if self.session.shape() != before { crate::app::mutations::after_layout_change(self); }
        self.needs_redraw |= update.effects.has_data || update.effects.bell_any;
        if update.effects.bell_any { crate::app::lifecycle::apply_bell_policy(self); }
        update.rpc
    }
}
