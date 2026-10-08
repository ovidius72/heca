//! A pane's own lifecycle: float, close, select, and moving its cursor.

use super::docks::focus_navigable_dock;
use super::pick::{begin_pick, pane_candidates_with_stable_letters};
use crate::app::interaction::focused_pane_id;
use crate::app_state::{AppState, InputMode};
use crate::input::WmAction;
use heca_core::layout::types::{Point, Rectangle, Size};

/// Float the focused pane, or put it back into the tiling if it already floats. Where it floats and
/// where it goes back are the workspace's to know ([`Workspace::float_tiled_pane`],
/// [`Workspace::unfloat_pane`]).
///
/// [`Workspace::float_tiled_pane`]: heca_core::layout::Workspace::float_tiled_pane
/// [`Workspace::unfloat_pane`]: heca_core::layout::Workspace::unfloat_pane
pub fn handle_float(state: &mut AppState, _action: &WmAction) {
    let Some(pane_id) = focused_pane_id(state) else {
        return;
    };
    // Allocated before the workspace is borrowed; spent only if unfloating has to rebuild the
    // column this pane came from. A derived id could collide with a column that still exists.
    let new_column_id = heca_core::layout::ColumnId(state.session.next_id());
    if let Some(ws) = state.session.active_workspace_mut()
        && !ws.unfloat_pane(pane_id, new_column_id)
    {
        let rect = ws.default_float_rect();
        ws.float_tiled_pane(pane_id, rect);
    }
}

pub fn handle_float_at(state: &mut AppState, action: &WmAction) {
    let WmAction::FloatAt {
        pane_id,
        x,
        y,
        width,
        height,
    } = action
    else {
        return;
    };
    if let Some(ws) = state.session.active_workspace_mut() {
        let rect = Rectangle::new(Point::new(*x, *y), Size::new(*width, *height));
        ws.float_tiled_pane(*pane_id, rect);
    }
}

/// Close the currently focused pane.
///
/// Delegates to the floating or tiled close path based on the focused pane's
/// domain. After removal, destroys the workspace if it's empty and others
/// remain, or leaves it empty if it's the only one.
///
/// # Floating domain
///
/// - Removes the floating pane and its backend.
/// - Switches to `FocusDomain::Tiled` only when no floating panes remain.
/// - Focuses the last visited tiled pane (or syncs from session state).
///
/// # Tiled domain
///
/// - Removes the active pane from the active column.
/// - Removes its backend.
///
/// # Empty workspace handling
///
/// If the workspace becomes empty after closing and there are multiple
/// workspaces, the empty one is destroyed and focus switches. If it's
/// the only workspace, it's left empty — the user can repopulate it via
/// the normal split bindings: `prefix+Enter` (new pane in a new column)
/// or `prefix+v` (new pane in the current column). In an empty workspace,
/// either binding effectively creates the first pane again.
pub fn handle_close_pane(state: &mut AppState, _action: &WmAction) {
    let Some(pane_id) = focused_pane_id(state) else {
        return;
    };
    // Raw close on the focused pane (`ClosePaneById` handles tiled + floating + empty-workspace
    // cleanup). Confirmation is owned by the **central destructive gate** at the dispatch
    // chokepoint (`maybe_confirm_destructive`), which intercepts `ClosePane` before it reaches
    // this handler; so this runs only for direct handler-to-handler execution and must NOT
    // re-gate (that would double-confirm).
    handle_close_pane_by_id(state, &WmAction::ClosePaneById { pane_id });
}

/// Close a specific pane by ID (RPC-style).
///
/// Handles both tiled and floating panes. After removal, destroys the
/// workspace if it's empty and other workspaces remain. If it's the only
/// workspace, it stays empty — the user can repopulate it via the normal
/// split bindings: `prefix+Enter` (new pane in a new column) or `prefix+v`
/// (new pane in the current column). In an empty workspace, either binding
/// effectively creates the first pane again.
/// Close the pane with this id — **wherever it is**, not only in the workspace you are standing in.
///
/// The whole point of a by-id action is that the caller names a pane the keyboard is not on: the
/// sidebar, a context menu, the exposé and RPC all reach panes in other workspaces. This searched
/// `active_workspace_mut()` alone, so every one of those silently did nothing off the current
/// workspace — deleting from the exposé's first row worked and its second row did not.
///
/// It is the same act [`close_pane_by_id_anywhere`](crate::app::mutations::close_pane_by_id_anywhere)
/// already performed for a shell that exits on its own, which had the search right and the tidying
/// up (empty-workspace destruction, search state, backend teardown) with it. Two implementations of
/// one act, and the user-facing one was the poorer: now there is one.
pub fn handle_close_pane_by_id(state: &mut AppState, action: &WmAction) {
    let WmAction::ClosePaneById { pane_id } = action else {
        return;
    };
    crate::app::mutations::close_pane_by_id_anywhere(state, *pane_id);
}

pub fn handle_pane_select(state: &mut AppState, _action: &WmAction) {
    if crate::app::selection::has_pane_candidate_overflow(&state.session) {
        focus_navigable_dock(state);
        return;
    }
    let candidates = pane_candidates_with_stable_letters(state);
    begin_pick(state, InputMode::PaneSelect { candidates });
}

/// **Move a mounted container's cursor to the row named by `key`** — the generic form of a click
/// on a row, and of any other gesture that means "the cursor belongs here now".
///
/// It moves the cursor and nothing else. Activating the row, focusing a pane or handing the
/// keyboard back are separate acts with separate names; a gesture that wants one of those says so.
pub fn handle_cursor_to(state: &mut AppState, action: &WmAction) {
    let WmAction::CursorTo { mount, key } = action else {
        return;
    };
    crate::providers::move_provider_cursor(state, mount, key);
}
