//! Where the keyboard is: **the window tree**, and nothing else.
//!
//! There is one record of it — the tree's focus — and everything here either reads it or moves it
//! through the tree's own door. A press on a control, a dock taking the keyboard and a dialog
//! closing all move it there, and what the key rules need to know (is a dock in front of the panes,
//! which one) is asked of the tree each time. No copy is kept in the chrome store to fall out of
//! step with it.
//!
//! The store hears about it only as an announcement ([`settle_window_focus`]): the event plugins
//! subscribe to, and the memory of which dock held the keyboard last.

use crate::app_state::AppState;

/// **The dock the keyboard is in**, read off the tree: its mount id, or `None` when it is in the
/// page outside every dock (the panes, say) or nowhere.
///
/// A dialog or the palette over a dock does not take the dock from under it — closing it hands the
/// keyboard back — so while one is open this still names the dock it will return to.
pub(crate) fn focused_dock(state: &AppState) -> Option<String> {
    heca_grid_ui::page_scope(&state.window_root)
}

/// **Give the keyboard to the dock mounted as `mount`.**
///
/// Through the tree's own door: it takes the keyboard unless something inside the dock already
/// holds it, and goes back to the control it last held. A dock the tree does not have yet — its
/// region was only just revealed — is asked for again once the tree has been rebuilt
/// ([`settle_window_focus`]), so the request is not lost.
pub(crate) fn focus_dock(state: &mut AppState, mount: &str) {
    if heca_grid_ui::focus_scope(&mut state.window_root, mount) {
        state.chrome_state.clear_dock_focus_request();
    } else {
        state.chrome_state.request_dock_focus(mount.to_string());
    }
}

/// **Take the keyboard out of the dock mounted as `mount`**, leaving it with nobody, so the keys go
/// back to the panes. Cancels a request for that dock that has not landed yet.
pub(crate) fn release_dock(state: &mut AppState, mount: &str) {
    state.chrome_state.clear_dock_focus_request();
    heca_grid_ui::release_scope(&mut state.window_root, mount);
}

/// Make the window tree's keyboard single, land any dock request the tree was not ready for, and
/// tell the store where the keyboard is. Called once a frame after the tree has ticked, and after a
/// press, so what the key rules read is current.
pub(crate) fn settle_window_focus(state: &mut AppState) {
    heca_grid_ui::settle_focus(&mut state.window_root);
    if let Some(mount) = state.chrome_state.dock_focus_request() {
        if heca_grid_ui::focus_scope(&mut state.window_root, &mount) {
            state.chrome_state.clear_dock_focus_request();
        } else if state.chrome_host.provider(&mount).is_none() {
            // The dock went away before it ever appeared: nothing is left to take the keyboard.
            state.chrome_state.clear_dock_focus_request();
        }
    }
    let dock = focused_dock(state);
    state.chrome_state.announce_focused_container(dock);
}
