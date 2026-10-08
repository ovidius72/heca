//! **The host's selection model, driven by a terminal's grid** — entering selection mode, moving the
//! caret, following it with the viewport, and beginning a drag-selection at a cell.

use crate::app::selection_model::{SelectionOwner, SelectionRegion, SelectionSource};
use crate::app_state::{AppState, InputMode};
use crate::chrome::terminal::TerminalId;

/// The terminal the keyboard's selection acts on: the focused pane's.
fn focused_terminal(state: &AppState) -> Option<TerminalId> {
    state.backends.identity_of(state.focused_pane?)
}

pub(crate) fn enter_selection_mode_for_focused_terminal(state: &mut AppState) -> bool {
    let Some(terminal) = focused_terminal(state) else {
        return false;
    };
    let Some(snapshot) = state
        .backends
        .get_by_id(terminal)
        .and_then(|backend| backend.terminal_snapshot())
    else {
        return false;
    };

    // If a selection already exists for this pane, preserve it
    // (re-entering selection mode does not discard an existing selection).
    if let Some(active) = state.selection.active()
        && active.owner == SelectionOwner(terminal)
        && matches!(active.region, SelectionRegion::HostGrid { .. })
    {
        state.input_mode = InputMode::Selection;
        state.needs_redraw = true;
        return true;
    }

    // Place a caret at the terminal cursor position — do NOT start a selection.
    // The user begins selection explicitly with `v` or `Space`.
    state.selection.set_caret(
        SelectionOwner(terminal),
        visible_row_to_stable_row(&snapshot, snapshot.cursor.row),
        snapshot.cursor.col,
    );
    state.input_mode = InputMode::Selection;
    state.needs_redraw = true;
    true
}

pub(crate) fn move_focused_terminal_selection(
    state: &mut AppState,
    row_delta: isize,
    col_delta: isize,
) -> bool {
    let Some(terminal) = focused_terminal(state) else {
        return false;
    };
    let Some(snapshot) = state
        .backends
        .get_by_id(terminal)
        .and_then(|backend| backend.terminal_snapshot())
    else {
        return false;
    };

    let max_col = snapshot.cols.saturating_sub(1);

    // ── Clamp to scrollback content bounds ──
    // Absolute stable-row range: oldest content (smallest) to newest (largest).
    // Computed from snapshot invariants so the bounds are independent of
    // the current viewport offset:
    //   newest = viewport_top_stable_row + viewport_offset + rows - 1
    //   oldest = newest - scrollback_rows + 1
    let content_base = snapshot.viewport_top_stable_row
        + snapshot.viewport_offset as isize
        + snapshot.rows as isize;
    let min_stable = content_base - snapshot.scrollback_rows as isize; // oldest
    let max_stable = content_base - 1; // newest

    // Handle caret-only state: move the caret in stable-row space.
    if state.selection.is_caret() {
        if let Some((stable_row, col)) = state.selection.caret_pos() {
            let next_stable = stable_row
                .saturating_add(row_delta)
                .clamp(min_stable, max_stable);
            let next_col = col.saturating_add_signed(col_delta).min(max_col);
            state.selection.move_caret(next_stable, next_col);
            // Auto-scroll the viewport so the caret stays visible (tmux copy-mode
            // follows the cursor; Q4 stable-row coords). Scrolls only when the
            // caret moved outside the visible range.
            ensure_caret_visible(state, terminal, next_stable, &snapshot);
            state.needs_redraw = true;
            return true;
        }
        return false;
    }

    // Handle active selection: update the focus end in stable-row space.
    let (anchor_stable_row, anchor_col, focus_stable_row, focus_col) =
        match state.selection.active() {
            Some(active) if active.owner == SelectionOwner(terminal) => match active.region {
                SelectionRegion::HostGrid {
                    anchor_stable_row,
                    anchor_col,
                    focus_stable_row,
                    focus_col,
                } => (anchor_stable_row, anchor_col, focus_stable_row, focus_col),
                SelectionRegion::BackendNative => {
                    return false;
                }
            },
            _ => {
                let cursor_stable = visible_row_to_stable_row(&snapshot, snapshot.cursor.row);
                (
                    cursor_stable,
                    snapshot.cursor.col,
                    cursor_stable,
                    snapshot.cursor.col,
                )
            }
        };

    let next_stable = focus_stable_row
        .saturating_add(row_delta)
        .clamp(min_stable, max_stable);
    let next_col = focus_col.saturating_add_signed(col_delta).min(max_col);

    state.selection.begin(
        SelectionOwner(terminal),
        SelectionSource::KeyboardMode,
        SelectionRegion::HostGrid {
            anchor_stable_row,
            anchor_col,
            focus_stable_row,
            focus_col,
        },
    );
    state.selection.update_focus(next_stable, next_col);
    state.input_mode = InputMode::Selection;
    // Auto-scroll the viewport so the selection focus stays visible (Q4).
    ensure_caret_visible(state, terminal, next_stable, &snapshot);
    state.needs_redraw = true;
    true
}

/// Scroll the pane's viewport so `caret_stable_row` is visible (tmux copy-mode
/// follows the cursor; Q4 stable-row coords). No-op when the caret is already
/// inside the visible range `[viewport_top_stable_row, viewport_top_stable_row + rows)`.
///
/// When the caret moves beyond the visible bottom, the offset decreases toward
/// the live bottom; when it moves above the visible top, the offset increases
/// toward history. Uses the **immediate** (non-animated) path: caret-follow is
/// edge-by-edge tracking like tmux copy-mode, so each `j`/`k` scroll exactly 1
/// row and stays pinned to the caret. Animating here would accumulate drift
/// under rapid key presses (the animation re-targets from its previous target).
pub(crate) fn ensure_caret_visible(
    state: &mut AppState,
    terminal: TerminalId,
    caret_stable_row: isize,
    snapshot: &heca_core::backend::TerminalSnapshot,
) {
    let visible_top = snapshot.viewport_top_stable_row;
    // Visible bottom stable row is exclusive (the range is [top, top+rows)).
    let visible_bottom_exclusive = visible_top + snapshot.rows as isize;

    if caret_stable_row < visible_top {
        // Caret moved above the visible top: scroll toward history (increase offset).
        let delta = (visible_top - caret_stable_row) as i32;
        if let Some(backend) = state.backends.get_mut_by_id(terminal) {
            backend.scroll_viewport(delta);
        }
    } else if caret_stable_row >= visible_bottom_exclusive {
        // Caret moved below the visible bottom: scroll toward live bottom (decrease offset).
        let delta = (caret_stable_row - visible_bottom_exclusive + 1) as i32;
        if let Some(backend) = state.backends.get_mut_by_id(terminal) {
            // Negative delta moves toward the live bottom.
            backend.scroll_viewport(-delta);
        }
    }
    // Else: caret is inside the visible range → no scroll needed.
}

pub(super) fn begin_terminal_selection_at(
    state: &mut AppState,
    terminal: TerminalId,
    row: usize,
    col: usize,
    source: SelectionSource,
) {
    let Some(snapshot) = state
        .backends
        .get_by_id(terminal)
        .and_then(|backend| backend.terminal_snapshot())
    else {
        return;
    };
    let stable_row = visible_row_to_stable_row(&snapshot, row);
    state.selection.begin(
        SelectionOwner(terminal),
        source,
        SelectionRegion::HostGrid {
            anchor_stable_row: stable_row,
            anchor_col: col,
            focus_stable_row: stable_row,
            focus_col: col,
        },
    );
    // A mouse selection is a gesture, not a mode: Shift+drag highlights, release copies, and the
    // keyboard stays where it was. Selection mode is the keyboard's.
    state.needs_redraw = true;
}

pub(super) fn visible_row_to_stable_row(
    snapshot: &heca_core::backend::TerminalSnapshot,
    row: usize,
) -> isize {
    snapshot.viewport_top_stable_row + row as isize
}
