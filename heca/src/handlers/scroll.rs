//! Scrolling: a terminal's scrollback, a focused dock, the column strip, and search in scrollback.

use crate::app::terminal_host::{
    ensure_caret_visible, enter_selection_mode_for_focused_terminal,
    move_focused_terminal_selection,
};
use crate::app_state::{AppState, InputMode};
use crate::chrome::panes::terminal::Page;
use crate::input::WmAction;

/// How many rows a page of the keyboard's terminal is — the fallback grid's when there is no
/// snapshot.
fn focused_terminal_page_rows(state: &AppState) -> usize {
    state
        .keyboard_terminal()
        .and_then(|terminal| state.client.backends.get_by_id(terminal))
        .and_then(|b| b.terminal_snapshot())
        .map(|s| s.rows)
        .unwrap_or(crate::app::backend_factory::FALLBACK_TERMINAL_GRID.1)
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
    let lines = (amount * state.settings.terminal_wheel_scroll_lines) as isize;
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
    let lines = (amount * state.settings.terminal_wheel_scroll_lines) as isize;
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
    if let Some(terminal) = state.keyboard_terminal()
        && let Some(ref snapshot) = state
            .client
            .backends
            .get_by_id(terminal)
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
        ensure_caret_visible(state, terminal, oldest, snapshot);
    }
}

pub fn handle_scrollback_to_bottom(state: &mut AppState, _action: &WmAction) {
    if !matches!(state.input_mode, InputMode::Selection) {
        let _ = enter_selection_mode_for_focused_terminal(state);
    }
    // Scroll to the live bottom immediately, then move the caret to the
    // cursor position (newest terminal text, not the adjusted cursor row).
    if let Some(backend) = state.keyboard_backend_mut() {
        backend.scroll_to_bottom();
    }
    if let Some(terminal) = state.keyboard_terminal()
        && let Some(ref snapshot) = state
            .client
            .backends
            .get_by_id(terminal)
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
    if let Some(backend) = state.keyboard_backend_mut() {
        backend.scroll_to_bottom();
    }
    state.selection.clear();
    // Leaving the copy-mode session also ends that pane's scrollback search; other
    // panes keep theirs.
    if let Some(terminal) = state.keyboard_terminal() {
        // Its bar closes with it: leaving copy-mode ends the search the terminal was showing.
        if let Some(handle) = state.terminals.get(&terminal) {
            handle.close_search();
        }
    }
    if matches!(state.input_mode, InputMode::Selection) {
        state.input_mode = InputMode::Normal;
    }
}

/// **A scroll key goes where the keyboard is**, like every other key.
///
/// While the window holds the keyboard, the intent goes to the widget holding it and bubbles up:
/// a terminal pages its history, a scroll region its content — in a dock, an overlay or anything
/// placed later. One with nothing to scroll declines and the key does nothing; it never falls
/// through to a pane the user is not on. Returns whether the window held the keyboard.
///
/// The host only ever says *what* — "one page down, this axis". The widget owns its viewport and
/// its clamp, so the arithmetic lives in the widget, once.
fn scroll_where_the_keyboard_is(state: &mut AppState, intent: heca_grid_ui::WidgetIntent) -> bool {
    if !crate::app::tree_keys::window_holds_keyboard(state) {
        return false;
    }
    crate::chrome::deliver(state, &heca_grid_ui::Event::Widget(intent));
    true
}

pub fn handle_scroll_line_up(state: &mut AppState, _action: &WmAction) {
    let lines = state.settings.terminal_wheel_scroll_lines as i32;
    if let Some(backend) = state.keyboard_backend_mut() {
        backend.scroll_viewport(lines);
    }
}

pub fn handle_scroll_line_down(state: &mut AppState, _action: &WmAction) {
    let lines = -(state.settings.terminal_wheel_scroll_lines as i32);
    if let Some(backend) = state.keyboard_backend_mut() {
        backend.scroll_viewport(lines);
    }
}

pub fn handle_scroll_page_up(state: &mut AppState, _action: &WmAction) {
    page_keyboard_terminal(state, Page::Up);
}

pub fn handle_scroll_page_down(state: &mut AppState, _action: &WmAction) {
    page_keyboard_terminal(state, Page::Down);
}

pub fn handle_scroll_to_top(state: &mut AppState, _action: &WmAction) {
    page_keyboard_terminal(state, Page::Top);
}

pub fn handle_scroll_to_bottom(state: &mut AppState, _action: &WmAction) {
    page_keyboard_terminal(state, Page::Bottom);
}

/// **A paging key**: whatever holds the keyboard in the window pages itself; while the panes hold
/// it, the pane's terminal is paged.
fn page_keyboard_terminal(state: &mut AppState, page: Page) {
    if scroll_where_the_keyboard_is(state, page.intent()) {
        return;
    }
    if let Some(backend) = state.keyboard_backend_mut() {
        page_terminal(backend, page);
    }
}

/// **Page a terminal's history**, wherever it is placed. A page is the terminal's own height, and
/// the move honours `terminal_scroll_animations`: a page jump is a discrete move the eye should
/// follow. Caret-follow and selection paths stay immediate elsewhere so the caret never lags.
pub(crate) fn page_terminal(backend: &mut dyn heca_core::backend::PaneBackend, page: Page) {
    let rows = backend
        .terminal_snapshot()
        .map(|s| s.rows)
        .unwrap_or(crate::app::backend_factory::FALLBACK_TERMINAL_GRID.1) as i32;
    match page {
        Page::Up => backend.scroll_viewport_animated(rows),
        Page::Down => backend.scroll_viewport_animated(-rows),
        Page::Top => backend.scroll_to_top_animated(),
        Page::Bottom => backend.scroll_to_bottom_animated(),
    }
}

// The horizontal four have no terminal half — a terminal's history has one axis — so they reach
// only what holds the keyboard in the window. A key aimed at nothing is silent, not an error.

pub fn handle_scroll_page_left(state: &mut AppState, _action: &WmAction) {
    scroll_where_the_keyboard_is(state, heca_grid_ui::WidgetIntent::ScrollPageLeft);
}

pub fn handle_scroll_page_right(state: &mut AppState, _action: &WmAction) {
    scroll_where_the_keyboard_is(state, heca_grid_ui::WidgetIntent::ScrollPageRight);
}

pub fn handle_scroll_to_left_edge(state: &mut AppState, _action: &WmAction) {
    scroll_where_the_keyboard_is(state, heca_grid_ui::WidgetIntent::ScrollToLeftEdge);
}

pub fn handle_scroll_to_right_edge(state: &mut AppState, _action: &WmAction) {
    scroll_where_the_keyboard_is(state, heca_grid_ui::WidgetIntent::ScrollToRightEdge);
}

pub fn handle_scroll_to_offset(state: &mut AppState, action: &WmAction) {
    let WmAction::ScrollToOffset { rows } = action else {
        return;
    };
    if let Some(backend) = state.keyboard_backend_mut() {
        scroll_backend_to_offset(backend, *rows);
    }
}

/// **Scroll a terminal to `rows` above its live bottom**, never past the top of its history. The one
/// place that does it: the action for the focused pane and a click on any terminal's scrollbar
/// both come here.
pub(crate) fn scroll_backend_to_offset(
    backend: &mut dyn heca_core::backend::PaneBackend,
    rows: usize,
) {
    let Some(snapshot) = backend.terminal_snapshot() else {
        return;
    };
    let max_offset = snapshot.scrollback_rows.saturating_sub(snapshot.rows);
    let target = rows.min(max_offset);
    let delta = target as i32 - snapshot.viewport_offset as i32;
    if delta != 0 {
        backend.scroll_viewport(delta);
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
    if let Some(mut ws) = state.layout_mut().active_workspace_mut() {
        let step = ws.scroll().area().size.w * 0.25;
        ws.scroll_mut().scroll_view(sign * step);
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
