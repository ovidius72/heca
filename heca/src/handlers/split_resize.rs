//! Splitting a column, resizing columns and panes, and zooming a column.

use super::navigation::handle_focus_workspace;
use crate::app_state::AppState;
use crate::input::WmAction;
use crate::pane_name;
use heca_core::layout::{ColumnWidth, Pane as LayoutPane, PaneId};

pub fn handle_split_horizontal(state: &mut AppState, _action: &WmAction) {
    let active_ws = state.session.active_workspace_idx;
    let next_id = state.session.next_id();
    let pane = LayoutPane::new(PaneId(next_id), pane_name(PaneId(next_id)));
    let backend_id = PaneId(next_id);
    state.session.add_pane(pane, None, true);
    state.start_shell_in(backend_id, active_ws);
}

pub fn handle_split_vertical(state: &mut AppState, _action: &WmAction) {
    let active_ws = state.session.active_workspace_idx;
    let next_id = state.session.next_id();
    let pane = LayoutPane::new(PaneId(next_id), pane_name(PaneId(next_id)));
    let backend_id = PaneId(next_id);
    let col_idx = state
        .session
        .active_workspace()
        .map(|ws| ws.scrolling.active_column_idx)
        .unwrap_or(0);
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.add_pane_to_column(col_idx, None, pane, true);
    }
    state.start_shell_in(backend_id, active_ws);
}

pub fn handle_resize_increase(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.resize_active_column(0.05);
    }
}

pub fn handle_resize_decrease(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.resize_active_column(-0.05);
    }
}

pub fn handle_zoom_column(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.toggle_active_column_zoom();
    }
}

/// Zoom the column named by `(ws_idx, col_idx)` — [`WmAction::ZoomColumnAtIndex`].
///
/// Zoom is a property of the *active* column, so naming another one means making it active first.
/// That is part of this action rather than a step every caller repeats: "zoom that column" is one
/// intent, and a caller that forgot the activation would silently zoom the wrong column.
///
/// Switching workspace goes through `focus_workspace` rather than assigning the index, so everything
/// that follows a workspace change still happens.
pub fn handle_zoom_column_at_index(state: &mut AppState, action: &WmAction) {
    let WmAction::ZoomColumnAtIndex { ws_idx, col_idx } = action else {
        return;
    };
    if *ws_idx >= state.session.workspaces.len() {
        return;
    }
    if *ws_idx != state.session.active_workspace_idx {
        handle_focus_workspace(state, &WmAction::FocusWorkspace { ws_idx: *ws_idx });
    }
    if let Some(ws) = state.session.active_workspace_mut()
        && *col_idx < ws.scrolling.columns.len()
    {
        ws.scrolling.activate_column(*col_idx);
    } else {
        return;
    }
    handle_zoom_column(state, &WmAction::ZoomColumn);
}

pub fn handle_pane_height_increase(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        let col_idx = ws.scrolling.active_column_idx;
        if let Some(col) = ws.scrolling.columns.get_mut(col_idx) {
            let h = ws.scrolling.working_area.size.h;
            let gaps = ws.scrolling.options.gaps;
            col.resize_active_pane_height(40.0, h, gaps);
        }
    }
}

pub fn handle_pane_height_decrease(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.active_workspace_mut() {
        let col_idx = ws.scrolling.active_column_idx;
        if let Some(col) = ws.scrolling.columns.get_mut(col_idx) {
            let h = ws.scrolling.working_area.size.h;
            let gaps = ws.scrolling.options.gaps;
            col.resize_active_pane_height(-40.0, h, gaps);
        }
    }
}

pub fn handle_resize(state: &mut AppState, action: &WmAction) {
    let WmAction::Resize {
        target,
        amount,
        edge,
    } = action
    else {
        return;
    };
    if let Some(ws) = state.session.active_workspace_mut() {
        // **The target decides the axis.** A column is resized across, a pane down — there was an
        // `axis` argument saying so as well, and it could only ever repeat the target or name a
        // combination that silently did nothing (`column`+`y`, `pane`+`x`). A key that quietly does
        // nothing is worse than one that is refused, and the argument was never a choice
        // (Antonio, 2026-09-04).
        match target {
            crate::input::ResizeTarget::Column => {
                let delta_f = *amount / 1000.0;
                // **Which of the column's two edges**, the same question a pane answers. A column's
                // left edge is the right edge of the column before it, so `Left` moves that
                // boundary; the scrolling space owns what each edge means.
                match edge {
                    crate::input::ResizeEdge::Left => {
                        ws.scrolling.move_active_column_left_boundary(delta_f)
                    }
                    _ => ws.scrolling.resize_active_column(delta_f),
                }
            }
            crate::input::ResizeTarget::Pane => {
                let h = ws.scrolling.working_area.size.h;
                let gaps = ws.scrolling.options.gaps;
                if let Some(col) = ws.scrolling.active_column_mut() {
                    // A boundary and a direction, not "grow me": positive is down, whichever pane
                    // is active. `resize` is a *directional* verb — see `move_pane_boundary`.
                    //
                    // **Which of the pane's two edges** is the caller's to say. `Auto` is the edge
                    // the pane already owned. The column owns what each edge *means*; this only
                    // names one.
                    match edge {
                        crate::input::ResizeEdge::Top => {
                            col.move_active_pane_top_boundary(*amount, h, gaps)
                        }
                        _ => col.move_active_pane_boundary(*amount, h, gaps),
                    }
                }
            }
        }
    }
}

/// Resize a specific column's width (mouse divider-drag / RPC). `delta` is a
/// proportion delta (or fraction of the working width for fixed columns), so a
/// pixel drag maps as `dx / working_area.width`.
pub fn handle_resize_column_by(state: &mut AppState, action: &WmAction) {
    let WmAction::ResizeColumnBy { col_idx, delta } = action else {
        return;
    };
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.resize_column(*col_idx, *delta);
    }
}

/// Resize a specific stacked pane's height (mouse divider-drag / RPC). `delta`
/// is logical px (drag down ⇒ taller).
pub fn handle_resize_pane_height_by(state: &mut AppState, action: &WmAction) {
    let WmAction::ResizePaneHeightBy {
        col_idx,
        pane_idx,
        delta,
    } = action
    else {
        return;
    };
    if let Some(ws) = state.session.active_workspace_mut() {
        ws.scrolling.resize_pane_height(*col_idx, *pane_idx, *delta);
    }
}

pub fn handle_resize_to(state: &mut AppState, action: &WmAction) {
    let WmAction::ResizeTo {
        target,
        width,
        height,
    } = action
    else {
        return;
    };
    if let Some(ws) = state.session.active_workspace_mut() {
        match target {
            crate::input::ResizeTarget::Column => {
                if let Some(col) = ws.scrolling.active_column_mut() {
                    col.width = ColumnWidth::Fixed(*width);
                    ws.scrolling.update_all_column_widths();
                }
            }
            crate::input::ResizeTarget::Pane => {
                let h = ws.scrolling.working_area.size.h;
                let gaps = ws.scrolling.options.gaps;
                if let Some(col) = ws.scrolling.active_column_mut() {
                    let pane_idx = col.active_pane_idx;
                    if let Some(size) = col.pane_sizes.get_mut(pane_idx) {
                        size.h = *height;
                    }
                    col.compute_pane_sizes(h, gaps);
                }
            }
        }
    }
}
