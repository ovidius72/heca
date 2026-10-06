//! Swapping panes and columns: nothing is added or removed, two things trade places.

use crate::input::WmAction;
use crate::server::{Change, ServerCx};

fn laid_out(changed: bool) -> Vec<Change> {
    match changed {
        true => vec![Change::LayoutChanged],
        false => Vec::new(),
    }
}

/// Move the active column one place along; the window keeps it on screen.
fn move_active_column(cx: &mut ServerCx<'_>, to: impl FnOnce(usize, usize) -> Option<usize>) -> Vec<Change> {
    cx.change_asker_columns(|space, asker| {
        let count = space.columns.len();
        to(asker.column, count)
            .and_then(|to| space.move_column(asker.column, to))
            .into_iter()
            .collect()
    })
}

/// Move the active column one place left.
pub(super) fn swap_left(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    move_active_column(cx, |at, _| at.checked_sub(1))
}

/// Move the active column one place right.
pub(super) fn swap_right(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    move_active_column(cx, |at, count| (at + 1 < count).then_some(at + 1))
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
