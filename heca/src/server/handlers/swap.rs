//! Swapping panes and columns: nothing is added or removed, two things trade places.

use crate::input::WmAction;
use crate::server::{Change, ServerCx};

fn laid_out(changed: bool) -> Vec<Change> {
    match changed {
        true => vec![Change::LayoutChanged],
        false => Vec::new(),
    }
}

/// Move the active column one place left, and bring it into view.
pub(super) fn swap_left(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    cx.change_asker_workspace(|mut ws| {
        ws.scroll_mut().move_column_left();
        ws.scroll_mut().align_view_to_active_column();
    })
}

/// Move the active column one place right, and bring it into view.
pub(super) fn swap_right(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    cx.change_asker_workspace(|mut ws| {
        ws.scroll_mut().move_column_right();
        ws.scroll_mut().align_view_to_active_column();
    })
}

/// Swap the active pane with the one above it.
pub(super) fn swap_up(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    cx.change_asker_workspace(|mut ws| {
        ws.swap_active_pane_up();
    })
}

/// Swap the active pane with the one below it.
pub(super) fn swap_down(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    cx.change_asker_workspace(|mut ws| {
        ws.swap_active_pane_down();
    })
}

/// Swap two panes, wherever they are.
pub(super) fn swap(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::Swap { a_id, b_id } = action else {
        return Vec::new();
    };
    laid_out(cx.layout.swap_panes(*a_id, *b_id))
}

/// Swap two columns, wherever they are.
pub(super) fn swap_columns(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::SwapColumns {
        a_ws,
        a_col,
        b_ws,
        b_col,
    } = action
    else {
        return Vec::new();
    };
    laid_out(cx.layout.swap_columns_between(*a_ws, *a_col, *b_ws, *b_col))
}
