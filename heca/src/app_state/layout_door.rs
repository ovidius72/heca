//! The one door from a window to the layout and to the state every window shares.

use super::*;

/// What this window asked the server: the action, and the name it is known by.
#[derive(Clone, Copy)]
pub(crate) struct Asked<'a> {
    pub(crate) action: &'a crate::input::WmAction,
    pub(crate) name: &'a str,
    /// Where everything was, in every workspace, for this window when it asked.
    pub(crate) before: &'a heca_core::layout::Before,
}

impl AppState {
    /// The session as this window sees it — for everything that reads where things are.
    pub fn layout(&self) -> Layout<'_> {
        self.session.through(&self.view)
    }

    /// The session, moved by this window — for everything that changes where things are. A move
    /// shifts this window's scroll with it; nothing at the call site has to say so.
    pub fn layout_mut(&mut self) -> LayoutMut<'_> {
        self.session.through_mut(&mut self.view)
    }

    /// Do what only a window can about what the server says changed on its own.
    pub(crate) fn apply(&mut self, changes: Vec<crate::server::Change>) {
        self.apply_answer(changes, None);
    }

    /// Do what only a window can about what the server says changed. `asked` is the built-in this
    /// window sent, by name and as data, when the changes answer one.
    pub(crate) fn apply_answer(
        &mut self,
        changes: Vec<crate::server::Change>,
        asked: Option<Asked<'_>>,
    ) {
        for change in changes {
            if matches!(change, crate::server::Change::NotificationsChanged) {
                // The toast data rides its own message (`ServerMsg::Notifications`), applied in
                // `receive_client`; this dataless fact only asks for a redraw.
                self.needs_redraw = true;
                continue;
            }
            let mut layout = self.session.through_mut(&mut self.view);
            let mut tracking = super::reaction::Tracking {
                last_visited_ws: &mut self.last_visited_ws_idx,
                last_visited_pane_per_ws: &mut self.last_visited_pane_per_ws,
                expose_cursor_per_ws: &mut self.expose_cursor_per_ws,
            };
            let reaction = super::reaction::window_reacts(
                &mut layout,
                &mut tracking,
                &change,
                asked.map(|a| a.action),
                asked.map(|a| a.before),
            );
            self.needs_redraw |= reaction.redraw;
            if reaction.names_changed {
                crate::app::mutations::after_metadata_change(self);
            }
            if let (Some(reason), Some(asked)) = (reaction.refusal, asked) {
                self.status_note = Some(crate::handlers::act_refusal(
                    &self.action_catalog,
                    asked.name,
                    super::reaction::refusal_words(reason),
                ));
            }
        }
    }

    /// **Run a built-in action on the server**, for this window, and react to what changed. Only
    /// for an action the server runs ([`ServerState::runs`](crate::server::ServerState::runs)). The
    /// server gets the session and what this window says about itself, never its view.
    pub(crate) fn run_on_server(&mut self, action: &crate::input::WmAction, name: &str) {
        let asker = crate::server::Asker::seen_through(
            self.layout(),
            self.focused_pane,
            self.terminal_cell_size,
        );
        let before = self.layout().before();
        self.client.act(action.clone(), asker, name.into(), before);
    }

    /// Rebuild the toast stack's signal from the notification projection the server sent
    /// ([`notifications`](Self::notifications)), if it changed. Each view becomes a `ToastSpec`
    /// through the one window-side builder, so the cut and action wiring live in one place.
    pub(crate) fn rebuild_toasts(&mut self) {
        use heca_grid_ui::reactive::{SignalGet, SignalUpdate};
        let next: Vec<_> = self
            .notifications
            .iter()
            .map(crate::notification::toast_spec)
            .collect();
        if self.toasts.get_untracked() != next {
            self.toasts.set(next);
        }
    }
}
