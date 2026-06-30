//! Terminal host adapter between pane shells and terminal backends.
//!
//! This keeps terminal sizing and snapshot acquisition out of a specific pane
//! implementation so future `heca-grid-ui` pane shells can host terminals
//! through the same contract.

use crate::actions::ActionRegistry;
use crate::app::backend_store::BackendStore;
use crate::app::interaction::{InteractionSource, dispatch_action};
use crate::app::selection_model::{SelectionOwner, SelectionRegion, SelectionSource};
use crate::app_state::{AppState, InputMode, InteractiveMovePhase};

/// Default cell height in logical pixels for PixelDelta → line conversion
/// fallback when the terminal backend cannot be queried for real cell metrics.
const DEFAULT_CELL_H: f64 = 14.0;
use crate::input::WmAction;
use heca_core::backend::{
    BackendModifiers, BackendMouseButton, BackendMouseEvent, BackendMouseEventKind, PaneBackend,
    TerminalDamage, TerminalSnapshot,
};
use heca_core::layout::{PaneId, Point, Rectangle, Size};
use winit::event::{ElementState, MouseButton, MouseScrollDelta};

#[derive(Clone)]
pub(crate) struct TerminalMount {
    pub content_rect: Rectangle,
    pub snapshot: TerminalSnapshot,
    /// Pending visible damage for this pane's terminal content. Carried through
    /// the mount/render boundary even when the renderer still falls back to
    /// full redraw, so later retained-content work can consume real row damage
    /// without changing the host contract again.
    pub damage: TerminalDamage,
}

#[derive(Clone, Copy)]
struct TerminalInputTarget {
    pane_id: PaneId,
    content_rect: Rectangle,
}

pub(crate) fn prepare_terminal_mount(
    backends: &mut BackendStore,
    pane_id: PaneId,
    content_rect: Rectangle,
    base_cell_size: (f32, f32),
) -> Option<TerminalMount> {
    let backend = backends.get_mut(pane_id)?;
    sync_terminal_backend_size(backend, content_rect, base_cell_size);
    let snapshot = backend.terminal_snapshot()?;
    let damage = backend.take_terminal_damage();

    Some(TerminalMount {
        content_rect,
        snapshot,
        damage,
    })
}

fn sync_terminal_backend_size(
    backend: &mut dyn PaneBackend,
    content_rect: Rectangle,
    base_cell_size: (f32, f32),
) {
    let (approx_cell_w, approx_cell_h) = base_cell_size;
    if approx_cell_w <= 0.0 || approx_cell_h <= 0.0 {
        return;
    }

    let cols = fitted_grid_units(content_rect.size.w as f32, approx_cell_w);
    let rows = fitted_grid_units(content_rect.size.h as f32, approx_cell_h);
    let fitted_cell_w = (content_rect.size.w as f32 / cols as f32).max(1.0);
    let fitted_cell_h = (content_rect.size.h as f32 / rows as f32).max(1.0);

    backend.set_cell_size(fitted_cell_w, fitted_cell_h);
    backend.set_size(cols, rows);
}

fn fitted_grid_units(extent: f32, approx_cell: f32) -> usize {
    if extent <= 0.0 || approx_cell <= 0.0 {
        return 1;
    }

    if !extent.is_finite() || !approx_cell.is_finite() {
        return 1;
    }

    (extent / approx_cell).ceil().clamp(1.0, 16_384.0) as usize
}

pub(crate) fn forward_mouse_move(state: &mut AppState, pos: (f32, f32)) {
    if state.mouse.interactive_move.is_some() || state.mouse.drag_ctx.is_dragging() {
        return;
    }

    // ── Host selection drag: update focus ──
    //
    // If a host-owned selection is in progress (Selecting phase) and the
    // pointer is over the owning pane, update the selection focus to the
    // current cell coordinates. This makes Shift+drag feel like a
    // continuous selection gesture.
    //
    // We look up coordinates via `owner_id` (the pane the selection started
    // on), NOT the pane under the cursor. If the mouse drifts outside the
    // owning pane, `cell_coords_at_position` returns `None` for that pane's
    // content rect and the focus freezes — this is the correct UX: releasing
    // outside the pane still confirms the selection up to the last in-bounds
    // cell.
    if state.selection.is_selecting()
        && state.selection.source() == Some(SelectionSource::MouseDrag)
    {
        if let Some(SelectionOwner::Pane(owner_id)) = state.selection.owner()
            && let Some((row, col)) = cell_coords_at_position(state, owner_id, pos)
            && let Some(snapshot) = state
                .backends
                .get(owner_id)
                .and_then(|backend| backend.terminal_snapshot())
        {
            state
                .selection
                .update_focus(visible_row_to_stable_row(&snapshot, row), col);
            state.needs_redraw = true;
        }
        return;
    }

    let Some(target) = terminal_target_at_position(state, pos) else {
        return;
    };
    if state.focused_pane != Some(target.pane_id) {
        return;
    }

    let Some(event) = build_mouse_event(
        state,
        target,
        pos,
        BackendMouseEventKind::Move,
        BackendMouseButton::None,
    ) else {
        return;
    };
    if let Some(backend) = state.backends.get_mut(target.pane_id) {
        let _ = backend.process_mouse_event(&event);
    }
}

pub(crate) fn forward_mouse_button(
    state: &mut AppState,
    pos: (f32, f32),
    button: MouseButton,
    button_state: ElementState,
    registry: &ActionRegistry,
) {
    // Plain left-click (no Shift) while a selection is active: clear the selection
    // and exit selection mode. This is intentional — a plain click in a terminal
    // pane should clear any host selection so the user can resume normal terminal
    // interaction. If we instead placed the caret at the click position, the user
    // would be trapped in selection mode until they press Esc. The clear-on-click
    // behavior matches most terminal emulators (select text, click elsewhere to
    // deselect).
    if button == MouseButton::Left
        && button_state == ElementState::Pressed
        && !state.modifiers.shift_key()
        && state.selection.is_active()
    {
        state.selection.clear();
        if matches!(state.input_mode, InputMode::Selection) {
            state.input_mode = InputMode::Normal;
        }
        state.needs_redraw = true;
    }

    if started_interactive_move(state, button, button_state) || state.mouse.drag_ctx.is_dragging() {
        return;
    }

    // ── Host selection entry path: Shift + left-drag ──
    //
    // When the user holds Shift and presses/releases the left button over a
    // terminal pane, the event is routed to the shared host selection model
    // instead of being forwarded to the terminal backend. This preserves TUI
    // mouse behavior (plain left-click still goes to the terminal) while
    // giving the host an explicit entry gesture for selection.
    if button == MouseButton::Left && state.modifiers.shift_key() {
        // Handle release first — this must work even when the pointer
        // has left the owning pane (e.g. dragged outside and released).
        // Only mouse-drag selections are ended by mouse release;
        // keyboard- or RPC-started selections are unaffected.
        if button_state == ElementState::Released {
            if state.selection.is_selecting()
                && state.selection.source() == Some(SelectionSource::MouseDrag)
            {
                // Confirm the selection, then copy to clipboard via the
                // action registry (no registry bypass). Shift+drag is a
                // complete gesture: select → release → copy, like most
                // terminal emulators. The copy handler also clears the
                // selection and exits selection mode, so we don't set
                // InputMode here.
                state.selection.end();
                dispatch_action(
                    state,
                    registry,
                    InteractionSource::MouseContent,
                    &WmAction::CopySelection,
                );
                state.needs_redraw = true;
            }
            return;
        }

        // Pressed path: need a valid target to begin selection.
        let Some(target) = terminal_target_at_position(state, pos) else {
            return;
        };
        // Guard: only begin a HostGrid selection on panes whose backend
        // actually supports terminal snapshots (i.e. has a cell grid).
        // Future non-terminal panes (browser, Neovim GUI) will use
        // `BackendNative` selection or a different entry path.
        let has_terminal_grid = state
            .backends
            .get(target.pane_id)
            .and_then(|backend| backend.terminal_snapshot())
            .is_some();
        if !has_terminal_grid {
            return;
        }

        if let Some((row, col)) = cell_coords_at_position(state, target.pane_id, pos) {
            begin_terminal_selection_at(
                state,
                target.pane_id,
                row,
                col,
                SelectionSource::MouseDrag,
            );
        }
        return;
    }

    let Some(target) = terminal_target_at_position(state, pos) else {
        return;
    };
    let kind = match button_state {
        ElementState::Pressed => BackendMouseEventKind::Press,
        ElementState::Released => BackendMouseEventKind::Release,
    };
    let Some(button) = map_mouse_button(button) else {
        return;
    };
    let Some(event) = build_mouse_event(state, target, pos, kind, button) else {
        return;
    };
    if let Some(backend) = state.backends.get_mut(target.pane_id) {
        let _ = backend.process_mouse_event(&event);
    }
}

pub(crate) fn forward_mouse_wheel(state: &mut AppState, pos: (f32, f32), delta: MouseScrollDelta) {
    if state.mouse.interactive_move.is_some() || state.mouse.drag_ctx.is_dragging() {
        return;
    }

    let Some(target) = terminal_target_at_position(state, pos) else {
        return;
    };

    // ── Host scrollback routing ──
    //
    // Wheel → host scrollback UNLESS the terminal has mouse-grab active.
    // Shift+wheel → always host scrollback (bypasses any grab).
    //
    // Q2 policy:
    //   `shift_held`         → always host scroll (bypasses grab)
    //   `terminal_mouse && !grab` → host scroll
    //   `terminal_mouse && grab`  → forward to terminal
    //   `!terminal_mouse`         → forward to terminal
    //
    // Q3: scrolling up at the live bottom enters Selection mode so further
    // scroll keys (u/d/Ctrl-u/Ctrl-d/g/G) work immediately without requiring
    // a separate `prefix+q` or `esc` toggle.
    let shift_held = state.modifiers.shift_key();
    let wants_mouse = state
        .backends
        .get(target.pane_id)
        .is_some_and(|b| b.is_mouse_grabbed());
    let do_host_scroll = shift_held || (state.terminal_mouse_enabled && !wants_mouse);

    if !do_host_scroll {
        // Forward wheel to the terminal backend.
        forward_wheel_to_terminal(state, target, pos, delta);
        return;
    }

    // Host scrollback fallback (`terminal-task-01h`): if the host viewport has
    // no room to move (the backend is in an alternate screen with no retained
    // history, e.g. `nvim`/`less`, or the pane simply has no scrollback yet), the
    // wheel is otherwise wasted. For plain wheel we gracefully forward it to the
    // terminal so non-grabbed TUIs can still react. For `Shift+wheel` this is a
    // documented no-op: Shift's contract is "bypass the TUI, host-only", so we
    // never silently scroll the TUI. wezterm does not expose the main screen's
    // preserved history while the alt screen is active, so host scrollback in
    // alt-screen TUIs is an inherent limitation (see README).
    let host_can_scroll = state
        .backends
        .get(target.pane_id)
        .and_then(|b| b.terminal_snapshot())
        .is_some_and(|s| s.scrollback_rows > s.rows);
    if !host_can_scroll {
        if shift_held {
            // Documented limitation: Shift+wheel host scrollback is a no-op
            // while the backend is in an alternate screen (no exposed host
            // history).
            return;
        }
        forward_wheel_to_terminal(state, target, pos, delta);
        return;
    }

    let lines = state.terminal_wheel_scroll_lines;
    // Determine number of host-scroll notches from the dominant wheel axis.
    // On many platforms Shift+wheel is remapped to horizontal scroll (`x`) with
    // `y == 0`; the host scrollback policy still wants that gesture to behave as
    // a vertical history scroll, so we fall back to `x` when there is no usable
    // vertical component.
    let signed_notches = host_scroll_notches(
        delta,
        state
            .backends
            .get(target.pane_id)
            .map(|b| b.cell_size().1 as f64)
            .unwrap_or(DEFAULT_CELL_H),
    );
    let total = (signed_notches.abs().ceil() as usize) * lines;
    if total == 0 {
        return;
    }
    let delta_i32: i32 = if signed_notches > 0.0 {
        total as i32
    } else {
        -(total as i32)
    };

    if let Some(backend) = state.backends.get_mut(target.pane_id) {
        let before_offset = backend
            .terminal_snapshot()
            .map(|snapshot| snapshot.viewport_offset)
            .unwrap_or(0);
        let was_at_bottom = before_offset == 0;
        backend.scroll_viewport(delta_i32);
        let after_offset = backend
            .terminal_snapshot()
            .map(|snapshot| snapshot.viewport_offset)
            .unwrap_or(before_offset);
        let moved = after_offset != before_offset;
        // Q3: scrolling up from the live bottom enters Selection mode, but only
        // if the host viewport actually moved. Alt-screen/no-history cases like
        // `less` would otherwise spuriously enter Selection mode on a no-op wheel.
        if signed_notches > 0.0 && was_at_bottom && moved {
            state.input_mode = InputMode::Selection;
        }
    }
    state.needs_redraw = true;
    // Reset prefix timeout on scroll (like keyboard input).
    state.prefix_entered_at = None;
}

pub(crate) fn notify_focus_changed(
    state: &mut AppState,
    prev: Option<PaneId>,
    next: Option<PaneId>,
) {
    if prev == next {
        return;
    }

    if let Some(prev) = prev
        && Some(prev) != next
        && let Some(backend) = state.backends.get_mut(prev)
    {
        backend.focus_changed(false);
    }

    if let Some(next) = next
        && Some(next) != prev
        && let Some(backend) = state.backends.get_mut(next)
    {
        backend.focus_changed(true);
    }
}

pub(crate) fn notify_window_focus_changed(state: &mut AppState, focused: bool) {
    let Some(pane_id) = state.focused_pane else {
        return;
    };
    if let Some(backend) = state.backends.get_mut(pane_id) {
        backend.focus_changed(focused);
    }
}

pub(crate) fn should_intercept_selection_gesture(
    state: &AppState,
    pos: (f32, f32),
    button: MouseButton,
    button_state: ElementState,
) -> bool {
    if button != MouseButton::Left
        || button_state != ElementState::Pressed
        || !state.modifiers.shift_key()
    {
        return false;
    }
    let move_modifier_held = match state.interactive_move_modifier {
        heca_config::theme::ModifierKey::Super => state.modifiers.super_key(),
        heca_config::theme::ModifierKey::Alt => state.modifiers.alt_key(),
        heca_config::theme::ModifierKey::Ctrl => state.modifiers.control_key(),
        heca_config::theme::ModifierKey::Shift => state.modifiers.shift_key(),
    };
    if move_modifier_held {
        return false;
    }
    let Some(target) = terminal_target_at_position(state, pos) else {
        return false;
    };
    state
        .backends
        .get(target.pane_id)
        .and_then(|backend| backend.terminal_snapshot())
        .is_some()
}

pub(crate) fn enter_selection_mode_for_focused_terminal(state: &mut AppState) -> bool {
    let Some(pane_id) = state.focused_pane else {
        return false;
    };
    let Some(snapshot) = state
        .backends
        .get(pane_id)
        .and_then(|backend| backend.terminal_snapshot())
    else {
        return false;
    };

    // If a selection already exists for this pane, preserve it
    // (re-entering selection mode does not discard an existing selection).
    if let Some(active) = state.selection.active()
        && active.owner == SelectionOwner::Pane(pane_id)
        && matches!(active.region, SelectionRegion::HostGrid { .. })
    {
        state.input_mode = InputMode::Selection;
        state.needs_redraw = true;
        return true;
    }

    // Place a caret at the terminal cursor position — do NOT start a selection.
    // The user begins selection explicitly with `v` or `Space`.
    state.selection.set_caret(
        SelectionOwner::Pane(pane_id),
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
    let Some(pane_id) = state.focused_pane else {
        return false;
    };
    let Some(snapshot) = state
        .backends
        .get(pane_id)
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
    let content_base =
        snapshot.viewport_top_stable_row + snapshot.viewport_offset as isize + snapshot.rows as isize;
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
            ensure_caret_visible(state, pane_id, next_stable, &snapshot);
            state.needs_redraw = true;
            return true;
        }
        return false;
    }

    // Handle active selection: update the focus end in stable-row space.
    let (anchor_stable_row, anchor_col, focus_stable_row, focus_col) =
        match state.selection.active() {
            Some(active) if active.owner == SelectionOwner::Pane(pane_id) => match active.region {
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
        SelectionOwner::Pane(pane_id),
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
    ensure_caret_visible(state, pane_id, next_stable, &snapshot);
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
    pane_id: heca_core::layout::PaneId,
    caret_stable_row: isize,
    snapshot: &heca_core::backend::TerminalSnapshot,
) {
    let visible_top = snapshot.viewport_top_stable_row;
    // Visible bottom stable row is exclusive (the range is [top, top+rows)).
    let visible_bottom_exclusive = visible_top + snapshot.rows as isize;

    if caret_stable_row < visible_top {
        // Caret moved above the visible top: scroll toward history (increase offset).
        let delta = (visible_top - caret_stable_row) as i32;
        if let Some(backend) = state.backends.get_mut(pane_id) {
            backend.scroll_viewport(delta);
        }
    } else if caret_stable_row >= visible_bottom_exclusive {
        // Caret moved below the visible bottom: scroll toward live bottom (decrease offset).
        let delta = (caret_stable_row - visible_bottom_exclusive + 1) as i32;
        if let Some(backend) = state.backends.get_mut(pane_id) {
            // Negative delta moves toward the live bottom.
            backend.scroll_viewport(-delta);
        }
    }
    // Else: caret is inside the visible range → no scroll needed.
}

fn build_mouse_event(
    state: &AppState,
    target: TerminalInputTarget,
    pos: (f32, f32),
    kind: BackendMouseEventKind,
    button: BackendMouseButton,
) -> Option<BackendMouseEvent> {
    let (cell_w, cell_h) = state
        .backends
        .get(target.pane_id)
        .map(|backend| backend.cell_size())
        .unwrap_or(state.terminal_cell_size);
    let (row, col) = cell_coords_in_rect(pos, target.content_rect, cell_w as f64, cell_h as f64)?;

    let local_x = pos.0 as f64 - target.content_rect.loc.x;
    let local_y = pos.1 as f64 - target.content_rect.loc.y;
    let x_pixel_offset = (local_x - (col as f64 * cell_w as f64)).round() as isize;
    let y_pixel_offset = (local_y - (row as f64 * cell_h as f64)).round() as isize;

    Some(BackendMouseEvent {
        kind,
        col,
        row,
        x_pixel_offset,
        y_pixel_offset,
        button,
        modifiers: BackendModifiers {
            ctrl: state.modifiers.control_key(),
            shift: state.modifiers.shift_key(),
            alt: state.modifiers.alt_key(),
            super_: state.modifiers.super_key(),
        },
    })
}

fn begin_terminal_selection_at(
    state: &mut AppState,
    pane_id: PaneId,
    row: usize,
    col: usize,
    source: SelectionSource,
) {
    let Some(snapshot) = state
        .backends
        .get(pane_id)
        .and_then(|backend| backend.terminal_snapshot())
    else {
        return;
    };
    let stable_row = visible_row_to_stable_row(&snapshot, row);
    state.selection.begin(
        SelectionOwner::Pane(pane_id),
        source,
        SelectionRegion::HostGrid {
            anchor_stable_row: stable_row,
            anchor_col: col,
            focus_stable_row: stable_row,
            focus_col: col,
        },
    );
    state.input_mode = InputMode::Selection;
    state.needs_redraw = true;
}

fn visible_row_to_stable_row(snapshot: &heca_core::backend::TerminalSnapshot, row: usize) -> isize {
    snapshot.viewport_top_stable_row + row as isize
}

fn started_interactive_move(
    state: &AppState,
    button: MouseButton,
    button_state: ElementState,
) -> bool {
    matches!(
        (button, button_state, &state.mouse.interactive_move),
        (
            MouseButton::Left,
            ElementState::Pressed,
            Some(InteractiveMovePhase::Starting { .. } | InteractiveMovePhase::Moving { .. })
        )
    )
}

/// Convert a pointer position to terminal cell coordinates within a content
/// rect, given cell dimensions.
///
/// Returns `None` if the position is outside the rect or if cell metrics are
/// invalid. This is the single geometry path shared by `build_mouse_event`
/// (terminal forwarding) and `cell_coords_at_position` (host selection) so
/// both paths use one coordinate model.
fn cell_coords_in_rect(
    pos: (f32, f32),
    content_rect: Rectangle,
    cell_w: f64,
    cell_h: f64,
) -> Option<(usize, usize)> {
    let local_x = pos.0 as f64 - content_rect.loc.x;
    let local_y = pos.1 as f64 - content_rect.loc.y;
    if local_x < 0.0
        || local_y < 0.0
        || local_x >= content_rect.size.w
        || local_y >= content_rect.size.h
    {
        return None;
    }
    if cell_w <= 0.0 || cell_h <= 0.0 {
        return None;
    }
    let col = (local_x / cell_w).floor().max(0.0) as usize;
    let row = (local_y / cell_h).floor().max(0.0) as usize;
    Some((row, col))
}

/// Convert a pointer position to terminal cell coordinates for a given pane.
///
/// Resolves the pane's content rect and cell metrics, then delegates to
/// `cell_coords_in_rect` — the single geometry path shared with
/// `build_mouse_event`.
fn cell_coords_at_position(
    state: &AppState,
    pane_id: PaneId,
    pos: (f32, f32),
) -> Option<(usize, usize)> {
    let content_rect = content_rect_for_pane(state, pane_id)?;
    let (cell_w, cell_h) = state
        .backends
        .get(pane_id)
        .map(|backend| backend.cell_size())
        .unwrap_or(state.terminal_cell_size);
    cell_coords_in_rect(pos, content_rect, cell_w as f64, cell_h as f64)
}

/// If the pointer at `pos` lands on a captured hyperlink cell within `pane_id`,
/// return its target URI.
///
/// Resolves the cell under the pointer (`cell_coords_at_position`) and looks it
/// up against the pane's `snapshot.hyperlinks` (OSC 8 + auto-detected, same
/// pipeline). `start_col` is inclusive and `end_col` is exclusive, matching the
/// capture/renderer contract. Used by the Cmd+click open-link surface
/// (`terminal-task-18`).
pub(crate) fn hyperlink_uri_at_position(
    state: &AppState,
    pane_id: PaneId,
    pos: (f32, f32),
) -> Option<String> {
    let (row, col) = cell_coords_at_position(state, pane_id, pos)?;
    let snapshot = state
        .backends
        .get(pane_id)
        .and_then(|backend| backend.terminal_snapshot())?;
    hyperlink_at_cell(&snapshot.hyperlinks, row, col).map(str::to_owned)
}

/// Build the follow-link candidates across **all visible panes**: one labelled
/// keycap per visible hyperlink span (OSC 8 + auto-detected, same pipeline),
/// assigned letters sequentially (a–z A–Z, shared 52-letter cap) in visible-pane
/// order. Each candidate carries its own `pane_id`. Panes fully off-screen and
/// panes without a terminal backend are skipped. terminal-task-18.
pub(crate) fn collect_link_hints(state: &AppState) -> Vec<crate::app_state::LinkHint> {
    let (win_w, win_h) = window_logical_size(state);
    let mut hints = Vec::new();
    let mut idx = 0usize;
    for (pane_id, x, y, w, h) in pane_outer_frames(state) {
        // Skip panes scrolled fully off-screen — their keycaps would be culled and
        // would only waste labels.
        if x + w <= 0.0 || y + h <= 0.0 || x >= win_w || y >= win_h {
            continue;
        }
        let Some(snapshot) = state
            .backends
            .get(pane_id)
            .and_then(|backend| backend.terminal_snapshot())
        else {
            continue;
        };
        for span in &snapshot.hyperlinks {
            let Some(label) = crate::app::selection::candidate_letter(idx) else {
                return hints; // 52-label cap reached.
            };
            hints.push(crate::app_state::LinkHint {
                label,
                pane_id,
                row: span.row,
                start_col: span.start_col,
                url: span.uri.clone(),
            });
            idx += 1;
        }
    }
    hints
}

/// Screen position (logical px, top-left) of cell `(row, col)` in `pane_id`'s
/// terminal content, or `None` if the pane has no resolvable content rect. The
/// inverse of `cell_coords_at_position`; used to stamp follow-link keycaps over a
/// link's first cell.
pub(crate) fn cell_screen_pos(
    state: &AppState,
    pane_id: PaneId,
    row: usize,
    col: usize,
) -> Option<(f32, f32)> {
    let content_rect = content_rect_for_pane(state, pane_id)?;
    let (cell_w, cell_h) = state
        .backends
        .get(pane_id)
        .map(|backend| backend.cell_size())
        .unwrap_or(state.terminal_cell_size);
    Some((
        content_rect.loc.x as f32 + col as f32 * cell_w,
        content_rect.loc.y as f32 + row as f32 * cell_h,
    ))
}

/// Target URI of the hyperlink at a **stable-row** cell in `pane_id`, if any.
///
/// Selection state is keyed by stable rows (history-stable), while hyperlink
/// spans are indexed by visible viewport row; this converts via
/// `stable - viewport_top_stable_row` and returns `None` when the cell is
/// scrolled out of the visible range. Used by follow-link-at-caret (`O` in
/// selection mode). terminal-task-18.
pub(crate) fn hyperlink_uri_at_stable_cell(
    state: &AppState,
    pane_id: PaneId,
    stable_row: isize,
    col: usize,
) -> Option<String> {
    let snapshot = state
        .backends
        .get(pane_id)
        .and_then(|backend| backend.terminal_snapshot())?;
    let visible_row = stable_row - snapshot.viewport_top_stable_row;
    if visible_row < 0 || visible_row >= snapshot.rows as isize {
        return None;
    }
    hyperlink_at_cell(&snapshot.hyperlinks, visible_row as usize, col).map(str::to_owned)
}

/// Find the hyperlink span covering cell `(row, col)`, if any.
///
/// `start_col` is inclusive, `end_col` exclusive (the capture/renderer
/// contract). The first matching span wins; OSC 8 capture never overlaps spans
/// and auto-detection skips cells already linked, so at most one matches.
fn hyperlink_at_cell(
    hyperlinks: &[heca_core::backend::HyperlinkSpan],
    row: usize,
    col: usize,
) -> Option<&str> {
    hyperlinks
        .iter()
        .find(|span| span.row == row && col >= span.start_col && col < span.end_col)
        .map(|span| span.uri.as_str())
}

fn map_mouse_button(button: MouseButton) -> Option<BackendMouseButton> {
    match button {
        MouseButton::Left => Some(BackendMouseButton::Left),
        MouseButton::Middle => Some(BackendMouseButton::Middle),
        MouseButton::Right => Some(BackendMouseButton::Right),
        MouseButton::Back | MouseButton::Forward | MouseButton::Other(_) => None,
    }
}

fn wheel_buttons(delta: MouseScrollDelta) -> Vec<BackendMouseButton> {
    match delta {
        MouseScrollDelta::LineDelta(x, y) => axis_wheel_buttons(x as f64, y as f64),
        MouseScrollDelta::PixelDelta(pos) => axis_wheel_buttons(pos.x, pos.y),
    }
}

/// Forward a wheel scroll to the terminal backend as press events.
///
/// Used by [`forward_mouse_wheel`] both when the policy routes the wheel away
/// from host scrollback (grabbed TUI / `terminal_mouse = false`) and as a
/// graceful fallback when the host viewport has no scrollback room (see
/// `terminal-task-01h`).
fn forward_wheel_to_terminal(
    state: &mut AppState,
    target: TerminalInputTarget,
    pos: (f32, f32),
    delta: MouseScrollDelta,
) {
    for button in wheel_buttons(delta) {
        let Some(event) =
            build_mouse_event(state, target, pos, BackendMouseEventKind::Press, button)
        else {
            continue;
        };
        if let Some(backend) = state.backends.get_mut(target.pane_id) {
            let _ = backend.process_mouse_event(&event);
        }
    }
}

fn host_scroll_notches(delta: MouseScrollDelta, cell_h: f64) -> f64 {
    match delta {
        MouseScrollDelta::LineDelta(x, y) => {
            if y.abs() > 0.0 {
                y as f64
            } else {
                x as f64
            }
        }
        MouseScrollDelta::PixelDelta(pos) => {
            let primary = if pos.y.abs() > 0.0 { pos.y } else { pos.x };
            if primary.abs() > 0.0 && cell_h > 0.0 {
                primary / cell_h
            } else {
                0.0
            }
        }
    }
}

fn axis_wheel_buttons(x: f64, y: f64) -> Vec<BackendMouseButton> {
    let mut buttons = Vec::new();
    push_wheel_buttons(
        &mut buttons,
        y,
        BackendMouseButton::WheelUp,
        BackendMouseButton::WheelDown,
    );
    push_wheel_buttons(
        &mut buttons,
        x,
        BackendMouseButton::WheelRight,
        BackendMouseButton::WheelLeft,
    );
    buttons
}

fn push_wheel_buttons(
    buttons: &mut Vec<BackendMouseButton>,
    delta: f64,
    positive: impl Fn(usize) -> BackendMouseButton,
    negative: impl Fn(usize) -> BackendMouseButton,
) {
    let steps = delta.abs().round().max(1.0) as usize;
    if delta > 0.0 {
        buttons.push(positive(steps));
    } else if delta < 0.0 {
        buttons.push(negative(steps));
    }
}

fn terminal_target_at_position(state: &AppState, pos: (f32, f32)) -> Option<TerminalInputTarget> {
    let hovered_pane = crate::mouse::hit_test_pane(state, pos)?;
    let content_rect = content_rect_for_pane(state, hovered_pane)?;
    Some(TerminalInputTarget {
        pane_id: hovered_pane,
        content_rect,
    })
}

/// The **outer** screen rect (full pane, before content inset) of every visible
/// pane — tiled then floating — in the active workspace. Same geometry the render
/// loop derives per pane (`render.rs`); kept here so the pane-header sync step can
/// position the in-pane info bar without a GPU borrow (render's `scene_view` holds
/// `state.compositor`). Returns `(pane_id, x, y, w, h)` in logical px.
pub(crate) fn pane_outer_frames(state: &AppState) -> Vec<(PaneId, f32, f32, f32, f32)> {
    let (win_w, win_h) = window_logical_size(state);
    let chrome = chrome_config(state);
    let pane_area = chrome.content_rect(win_w, win_h);
    let ws_offset = state
        .session
        .workspace_geometries()
        .first()
        .map(|(_, rect)| (rect.loc.x as f32, rect.loc.y as f32))
        .unwrap_or((0.0, 0.0));
    let mut frames = Vec::new();
    let Some(ws) = state.session.active_workspace() else {
        return frames;
    };
    for (pane_id, rect) in ws.scrolling.panes_with_positions() {
        let x = pane_area.loc.x as f32 + ws_offset.0 + rect.loc.x as f32;
        let y = pane_area.loc.y as f32 + ws_offset.1 + rect.loc.y as f32;
        frames.push((pane_id, x, y, rect.size.w as f32, rect.size.h as f32));
    }
    for float in &ws.floating_panes {
        let x = pane_area.loc.x as f32 + ws_offset.0 + float.position.x as f32;
        let y = pane_area.loc.y as f32 + ws_offset.1 + float.position.y as f32;
        frames.push((
            float.pane.id,
            x,
            y,
            float.size.w as f32,
            float.size.h as f32,
        ));
    }
    frames
}

fn content_rect_for_pane(state: &AppState, pane_id: PaneId) -> Option<Rectangle> {
    let (win_w, win_h) = window_logical_size(state);
    let chrome = chrome_config(state);
    let pane_area = chrome.content_rect(win_w, win_h);
    let ws_offset = state
        .session
        .workspace_geometries()
        .first()
        .map(|(_, rect)| (rect.loc.x as f32, rect.loc.y as f32))
        .unwrap_or((0.0, 0.0));
    if let Some(ws) = state.session.active_workspace() {
        for float in &ws.floating_panes {
            if float.pane.id != pane_id {
                continue;
            }
            let x = pane_area.loc.x as f32 + ws_offset.0 + float.position.x as f32;
            let y = pane_area.loc.y as f32 + ws_offset.1 + float.position.y as f32;
            let w = float.size.w as f32;
            let h = float.size.h as f32;
            let inset = pane_content_inset(state);
            let extra_top = crate::app::terminal_render::pane_title_top_inset(state);
            return inset_content_rect(x, y, w, h, inset, extra_top);
        }
    }

    for (candidate, rect) in state
        .session
        .active_workspace()
        .map(|ws| ws.scrolling.panes_with_positions())
        .unwrap_or_default()
    {
        if candidate != pane_id {
            continue;
        }
        let x = pane_area.loc.x as f32 + ws_offset.0 + rect.loc.x as f32;
        let y = pane_area.loc.y as f32 + ws_offset.1 + rect.loc.y as f32;
        let w = rect.size.w as f32;
        let h = rect.size.h as f32;
        let inset = pane_content_inset(state);
        let extra_top = crate::app::terminal_render::pane_title_top_inset(state);
        return inset_content_rect(x, y, w, h, inset, extra_top);
    }

    None
}

fn inset_content_rect(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    border_width: f32,
    extra_top: f32,
) -> Option<Rectangle> {
    let inset = border_width.max(1.0);
    let content_w = (w - inset * 2.0).max(0.0);
    let content_h = (h - inset * 2.0 - extra_top).max(0.0);
    if content_w <= 0.0 || content_h <= 0.0 {
        return None;
    }

    Some(Rectangle::new(
        Point::new((x + inset) as f64, (y + inset + extra_top) as f64),
        Size::new(content_w as f64, content_h as f64),
    ))
}

fn pane_content_inset(state: &AppState) -> f32 {
    let border = state.appearance.effective_pane_border_width(&state.theme);
    let padding = state.appearance.effective_pane_padding(&state.theme);
    padding.max(border + 1.0)
}

fn chrome_config(state: &AppState) -> crate::chrome::ChromeConfig {
    crate::chrome::ChromeConfig {
        tab_bar_height: crate::chrome::DEFAULT_TAB_BAR_HEIGHT,
        status_bar_height: crate::chrome::DEFAULT_STATUS_BAR_HEIGHT,
        left_sidebar_width: if state.chrome_state.left_visible() {
            state.chrome_state.left_size()
        } else {
            crate::chrome::DEFAULT_COLLAPSED_SIDEBAR_WIDTH
        },
        right_sidebar_width: if state.chrome_state.right_visible() {
            state.chrome_state.right_size()
        } else {
            crate::chrome::DEFAULT_COLLAPSED_SIDEBAR_WIDTH
        },
        sidebar_gap: state.appearance.effective_sidebar_gap(&state.theme),
    }
}

fn window_logical_size(state: &AppState) -> (f32, f32) {
    let phys = state.window.inner_size();
    (
        phys.width as f32 / state.scale_factor as f32,
        phys.height as f32 / state.scale_factor as f32,
    )
}

#[cfg(test)]
mod tests {
    use super::hyperlink_at_cell;
    use heca_core::backend::HyperlinkSpan;

    fn span(row: usize, start_col: usize, end_col: usize, uri: &str) -> HyperlinkSpan {
        HyperlinkSpan {
            row,
            start_col,
            end_col,
            uri: uri.to_owned(),
        }
    }

    #[test]
    fn hit_test_matches_inclusive_start_and_exclusive_end() {
        let links = [span(2, 4, 9, "https://example.com")];
        // Inclusive start.
        assert_eq!(
            hyperlink_at_cell(&links, 2, 4),
            Some("https://example.com")
        );
        // Interior cell.
        assert_eq!(
            hyperlink_at_cell(&links, 2, 8),
            Some("https://example.com")
        );
        // end_col is exclusive: the cell at end_col is not part of the link.
        assert_eq!(hyperlink_at_cell(&links, 2, 9), None);
        // Just before the start.
        assert_eq!(hyperlink_at_cell(&links, 2, 3), None);
    }

    #[test]
    fn hit_test_is_row_specific() {
        let links = [span(2, 4, 9, "https://example.com")];
        // Right columns, wrong row.
        assert_eq!(hyperlink_at_cell(&links, 1, 5), None);
        assert_eq!(hyperlink_at_cell(&links, 3, 5), None);
    }

    #[test]
    fn hit_test_picks_the_covering_span_among_several() {
        let links = [
            span(0, 0, 3, "http://a"),
            span(0, 10, 14, "http://b"),
            span(5, 2, 6, "mailto:x@y.z"),
        ];
        assert_eq!(hyperlink_at_cell(&links, 0, 1), Some("http://a"));
        assert_eq!(hyperlink_at_cell(&links, 0, 12), Some("http://b"));
        assert_eq!(hyperlink_at_cell(&links, 5, 5), Some("mailto:x@y.z"));
        // Gap between spans on row 0.
        assert_eq!(hyperlink_at_cell(&links, 0, 7), None);
    }

    #[test]
    fn hit_test_empty_list_is_none() {
        assert_eq!(hyperlink_at_cell(&[], 0, 0), None);
    }
}
