//! **Terminal actions the window runs** — stepping one terminal's own font size, which is drawn
//! at this window's size. Typing into a terminal and ending one are the server's
//! (`server/handlers/terminal.rs`); all of them name their target by one rule
//! ([`BackendStore::target`](crate::app::backend_store::BackendStore::target)).

use crate::app_state::AppState;
use crate::input::WmAction;

/// **Step one terminal's own font** — bigger, smaller, or back to following the app.
///
/// A terminal or pane the action names is zoomed. Naming neither means the terminal the keyboard
/// acts on ([`AppState::keyboard_terminal`]): one placed in a dock or an overlay that holds it is
/// asked first and answers itself (it says so back with its id, which lands here again).
pub fn handle_pane_terminal_font_zoom(state: &mut AppState, action: &WmAction) {
    let WmAction::PaneTerminalFontZoom {
        pane_id,
        terminal,
        step,
    } = action
    else {
        return;
    };
    let named = pane_id.is_some() || terminal.is_some();
    if !named && crate::app::terminal_host::tree_answers(state, step.intent()) {
        return;
    }
    let id = match named {
        true => {
            let backends = &state.client.backends;
            backends
                .target(*pane_id, *terminal, None)
                .and_then(|t| backends.terminal_of(t))
        }
        false => state.keyboard_terminal(),
    };
    let Some(id) = id else {
        return;
    };
    let delta = super::system::font_zoom_delta(state, *step);
    crate::app::terminal_metrics::apply_terminal_font_zoom(state, id, delta);
}
