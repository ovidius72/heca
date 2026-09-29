//! Scrolling: a terminal's scrollback, a focused dock, the column strip, and search in scrollback.

use crate::app::terminal_host::{
    ensure_caret_visible, enter_selection_mode_for_focused_terminal,
    move_focused_terminal_selection,
};
use crate::app_state::{AppState, InputMode};
use crate::input::WmAction;

/// Get the approximate viewport page size for the focused terminal pane, in rows.
/// Falls back to a sensible default (24) when no snapshot is available.
fn focused_terminal_page_rows(state: &AppState) -> usize {
    state
        .focused_pane
        .and_then(|pane_id| {
            state
                .backends
                .get(pane_id)
                .and_then(|b| b.terminal_snapshot())
                .map(|s| s.rows)
        })
        .unwrap_or(24)
}

pub fn handle_scrollback_page_up(state: &mut AppState, _action: &WmAction) {
    let page_rows = focused_terminal_page_rows(state) as isize;
    if !matches!(state.input_mode, InputMode::Selection) {
        let _ = enter_selection_mode_for_focused_terminal(state);
    }
    let _ = move_focused_terminal_selection(state, -page_rows, 0);
}

pub fn handle_scrollback_page_down(state: &mut AppState, _action: &WmAction) {
    let page_rows = focused_terminal_page_rows(state) as isize;
    if !matches!(state.input_mode, InputMode::Selection) {
        let _ = enter_selection_mode_for_focused_terminal(state);
    }
    let _ = move_focused_terminal_selection(state, page_rows, 0);
}

pub fn handle_scrollback_line_up(state: &mut AppState, action: &WmAction) {
    let WmAction::ScrollbackLineUp { amount } = action else {
        return;
    };
    // `amount` is in notches; multiply by the user-configurable lines-per-notch.
    let lines = (amount * state.terminal_wheel_scroll_lines) as isize;
    if !matches!(state.input_mode, InputMode::Selection) {
        let _ = enter_selection_mode_for_focused_terminal(state);
    }
    let _ = move_focused_terminal_selection(state, -lines, 0);
}

pub fn handle_scrollback_line_down(state: &mut AppState, action: &WmAction) {
    let WmAction::ScrollbackLineDown { amount } = action else {
        return;
    };
    // `amount` is in notches; multiply by the user-configurable lines-per-notch.
    let lines = (amount * state.terminal_wheel_scroll_lines) as isize;
    if !matches!(state.input_mode, InputMode::Selection) {
        let _ = enter_selection_mode_for_focused_terminal(state);
    }
    let _ = move_focused_terminal_selection(state, lines, 0);
}

pub fn handle_scrollback_to_top(state: &mut AppState, _action: &WmAction) {
    if !matches!(state.input_mode, InputMode::Selection) {
        let _ = enter_selection_mode_for_focused_terminal(state);
    }
    // Move caret to the oldest scrollback content row.
    if let Some(pane_id) = state.focused_pane
        && let Some(ref snapshot) = state
            .backends
            .get(pane_id)
            .and_then(|b| b.terminal_snapshot())
    {
        let base = snapshot.viewport_top_stable_row
            + snapshot.viewport_offset as isize
            + snapshot.rows as isize;
        let oldest = base - snapshot.scrollback_rows as isize;
        if state.selection.is_caret() {
            state.selection.move_caret(oldest, 0);
        } else {
            state.selection.update_focus(oldest, 0);
        }
        ensure_caret_visible(state, pane_id, oldest, snapshot);
    }
}

pub fn handle_scrollback_to_bottom(state: &mut AppState, _action: &WmAction) {
    if !matches!(state.input_mode, InputMode::Selection) {
        let _ = enter_selection_mode_for_focused_terminal(state);
    }
    // Scroll to the live bottom immediately, then move the caret to the
    // cursor position (newest terminal text, not the adjusted cursor row).
    if let Some(pane_id) = state.focused_pane
        && let Some(backend) = state.backends.get_mut(pane_id)
    {
        backend.scroll_to_bottom();
    }
    if let Some(pane_id) = state.focused_pane
        && let Some(ref snapshot) = state
            .backends
            .get(pane_id)
            .and_then(|b| b.terminal_snapshot())
    {
        let cursor_stable = snapshot.viewport_top_stable_row + snapshot.cursor.row as isize;
        if state.selection.is_caret() {
            state
                .selection
                .move_caret(cursor_stable, snapshot.cursor.col);
        } else {
            state
                .selection
                .update_focus(cursor_stable, snapshot.cursor.col);
        }
    }
}

pub fn handle_exit_scrollback(state: &mut AppState, _action: &WmAction) {
    // Scroll to live bottom, clear selection, and exit selection mode.
    if let Some(pane_id) = state.focused_pane
        && let Some(backend) = state.backends.get_mut(pane_id)
    {
        backend.scroll_to_bottom();
    }
    state.selection.clear();
    // Leaving the copy-mode session also ends that pane's scrollback search; other
    // panes keep theirs.
    if let Some(pane) = state.search_target_pane() {
        state.clear_search(pane);
    }
    if matches!(state.input_mode, InputMode::Selection) {
        state.input_mode = InputMode::Normal;
    }
}

/// Route a scroll to the **focused chrome container** when one holds keyboard focus.
///
/// Returns `true` when chrome focus is set — which is what makes the caller stop, whether or not
/// anything in the tree acted: a container with nothing scrollable declines, and a declined intent
/// stops at the chrome rather than falling through to the pane (F003/P085/T352). Letting it fall
/// through would mean the pane scrolls under a key aimed at a dock, which is the surprise this whole
/// phase exists to remove.
///
/// The host only ever says *what* — "one page down, this axis". The `ScrollRegion` owns its viewport
/// and its clamp, so the arithmetic lives in the widget, once, for every container that will ever
/// nest one.
fn scroll_focused_dock(state: &mut AppState, intent: heca_grid_ui::WidgetIntent) -> bool {
    if state.chrome_state.focused_container().is_none() {
        return false;
    }
    crate::chrome::deliver(state, &heca_grid_ui::Event::Widget(intent));
    true
}

pub fn handle_scroll_line_up(state: &mut AppState, _action: &WmAction) {
    let lines = state.terminal_wheel_scroll_lines as i32;
    if let Some(pane_id) = state.focused_pane
        && let Some(backend) = state.backends.get_mut(pane_id)
    {
        backend.scroll_viewport(lines);
    }
}

pub fn handle_scroll_line_down(state: &mut AppState, _action: &WmAction) {
    let lines = -(state.terminal_wheel_scroll_lines as i32);
    if let Some(pane_id) = state.focused_pane
        && let Some(backend) = state.backends.get_mut(pane_id)
    {
        backend.scroll_viewport(lines);
    }
}

pub fn handle_scroll_page_up(state: &mut AppState, _action: &WmAction) {
    if scroll_focused_dock(state, heca_grid_ui::WidgetIntent::ScrollPageUp) {
        return;
    }
    let page_rows = focused_terminal_page_rows(state) as i32;
    if let Some(pane_id) = state.focused_pane
        && let Some(backend) = state.backends.get_mut(pane_id)
    {
        // Direct page jumps are the primary user-visible discrete scrollback
        // jump path in Normal mode, so they should honor
        // `terminal_scroll_animations`. Caret-follow / selection-mode paths stay
        // immediate elsewhere to avoid lagging the caret behind the content.
        backend.scroll_viewport_animated(page_rows);
    }
}

pub fn handle_scroll_page_down(state: &mut AppState, _action: &WmAction) {
    if scroll_focused_dock(state, heca_grid_ui::WidgetIntent::ScrollPageDown) {
        return;
    }
    let page_rows = -(focused_terminal_page_rows(state) as i32);
    if let Some(pane_id) = state.focused_pane
        && let Some(backend) = state.backends.get_mut(pane_id)
    {
        backend.scroll_viewport_animated(page_rows);
    }
}

pub fn handle_scroll_to_top(state: &mut AppState, _action: &WmAction) {
    if scroll_focused_dock(state, heca_grid_ui::WidgetIntent::ScrollToTop) {
        return;
    }
    if let Some(pane_id) = state.focused_pane
        && let Some(backend) = state.backends.get_mut(pane_id)
    {
        backend.scroll_to_top_animated();
    }
}

pub fn handle_scroll_to_bottom(state: &mut AppState, _action: &WmAction) {
    if scroll_focused_dock(state, heca_grid_ui::WidgetIntent::ScrollToBottom) {
        return;
    }
    if let Some(pane_id) = state.focused_pane
        && let Some(backend) = state.backends.get_mut(pane_id)
    {
        backend.scroll_to_bottom_animated();
    }
}

// The horizontal four have no terminal half — a pane's scrollback has one axis — so they are the
// focused container's alone. A key aimed at nothing is silent, not an error.

pub fn handle_scroll_page_left(state: &mut AppState, _action: &WmAction) {
    scroll_focused_dock(state, heca_grid_ui::WidgetIntent::ScrollPageLeft);
}

pub fn handle_scroll_page_right(state: &mut AppState, _action: &WmAction) {
    scroll_focused_dock(state, heca_grid_ui::WidgetIntent::ScrollPageRight);
}

pub fn handle_scroll_to_left_edge(state: &mut AppState, _action: &WmAction) {
    scroll_focused_dock(state, heca_grid_ui::WidgetIntent::ScrollToLeftEdge);
}

pub fn handle_scroll_to_right_edge(state: &mut AppState, _action: &WmAction) {
    scroll_focused_dock(state, heca_grid_ui::WidgetIntent::ScrollToRightEdge);
}

pub fn handle_scroll_to_offset(state: &mut AppState, action: &WmAction) {
    let WmAction::ScrollToOffset { rows } = action else {
        return;
    };
    if let Some(pane_id) = state.focused_pane
        && let Some(backend) = state.backends.get_mut(pane_id)
        && let Some(snapshot) = backend.terminal_snapshot()
    {
        let max_offset = snapshot.scrollback_rows.saturating_sub(snapshot.rows);
        let target = (*rows).min(max_offset);
        let delta = target as i32 - snapshot.viewport_offset as i32;
        if delta != 0 {
            backend.scroll_viewport(delta);
        }
    }
}

/// Pan the horizontal view left/right by a quarter of the viewport, to reach
/// column overflow / content scrolled past an edge. View-only (no focus change).
pub fn handle_scroll_view_left(state: &mut AppState, _action: &WmAction) {
    scroll_view_by(state, -1.0);
}

pub fn handle_scroll_view_right(state: &mut AppState, _action: &WmAction) {
    scroll_view_by(state, 1.0);
}

fn scroll_view_by(state: &mut AppState, sign: f64) {
    if let Some(ws) = state.session.active_workspace_mut() {
        let step = ws.scrolling.working_area.size.w * 0.25;
        ws.scrolling.scroll_view(sign * step);
    }
}

/// Selection-mode `O`: open the hyperlink under the selection caret. Resolves the
/// caret cell (caret-only or the moving focus endpoint of an active selection) →
/// the owning pane's snapshot hyperlink → the shared open path. Safe no-op when
/// there is no caret, no owning pane, or no link under it.
pub fn handle_search_scrollback(state: &mut AppState, _action: &WmAction) {
    crate::app::terminal_host::enter_scrollback_search(state);
}

pub fn handle_search_next_match(state: &mut AppState, _action: &WmAction) {
    crate::app::terminal_host::search_step(state, true);
}

pub fn handle_search_prev_match(state: &mut AppState, _action: &WmAction) {
    crate::app::terminal_host::search_step(state, false);
}
