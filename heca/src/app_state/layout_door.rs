//! The one door from a window to the layout and to the state every window shares.

use super::*;

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

    /// **Ask the server to do something, and react to what changed** — the one door from a window
    /// to the state every window shares. The server's clock is read here, not the asker's.
    pub(crate) fn ask_server(&mut self, action: crate::server::ServerAction) {
        let changes = self.server.execute(action, std::time::Instant::now());
        self.apply(changes);
    }

    /// Do what only a window can about what the server says changed.
    pub(crate) fn apply(&mut self, changes: Vec<crate::server::Change>) {
        for change in changes {
            if matches!(change, crate::server::Change::NotificationsChanged) {
                self.sync_toasts();
                self.needs_redraw = true;
                continue;
            }
            let mut layout = self.session.through_mut(&mut self.view);
            if super::reaction::window_reacts(&mut layout, &mut self.last_visited_ws_idx, &change) {
                self.needs_redraw = true;
            }
        }
    }

    /// **Run a built-in action on the server**, for this window, and react to what changed. Only
    /// for an action the server runs ([`ServerState::runs`](crate::server::ServerState::runs)).
    ///
    /// The context is built from the session and this window's view only, so the server's own
    /// state stays borrowable beside it.
    pub(crate) fn run_on_server(&mut self, action: &crate::input::WmAction) {
        let asker = crate::server::Asker {
            focused_pane: self.focused_pane,
        };
        let mut cx = crate::server::ServerCx {
            layout: self.session.through_mut(&mut self.view),
            asker,
        };
        let changes = self.server.run(&mut cx, action);
        self.apply(changes);
    }

    /// Point the toast stack's signal at what the server says is on show, if that changed.
    pub(crate) fn sync_toasts(&mut self) {
        use heca_grid_ui::reactive::{SignalGet, SignalUpdate};
        let next = self.server.visible_toasts();
        if self.toasts.get_untracked() != next {
            self.toasts.set(next);
        }
    }
}
