//! Splitting a column. (Resizing and zooming are the server's: `server/handlers/layout.rs`.)

use crate::app_state::AppState;
use crate::input::WmAction;
use crate::pane_name;
use heca_core::layout::{Pane as LayoutPane, PaneId};

pub fn handle_split_horizontal(state: &mut AppState, _action: &WmAction) {
    let active_ws = state.layout().active_workspace_idx();
    let next_id = state.session.next_id();
    let pane = LayoutPane::new(PaneId(next_id), pane_name(PaneId(next_id)));
    let backend_id = PaneId(next_id);
    state.layout_mut().add_pane(pane, None, true);
    state.start_shell_in(backend_id, active_ws);
}

pub fn handle_split_vertical(state: &mut AppState, _action: &WmAction) {
    let active_ws = state.layout().active_workspace_idx();
    let next_id = state.session.next_id();
    let pane = LayoutPane::new(PaneId(next_id), pane_name(PaneId(next_id)));
    let backend_id = PaneId(next_id);
    let col_idx = state.layout().active_workspace()
        .map(|ws| ws.scroll().active_column_idx())
        .unwrap_or(0);
    if let Some(mut ws) = state.layout_mut().active_workspace_mut() {
        ws.scroll_mut().add_pane_to_column(col_idx, None, pane, true);
    }
    state.start_shell_in(backend_id, active_ws);
}
