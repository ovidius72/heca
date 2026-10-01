//! Terminal host adapter between pane shells and terminal backends.
//!
//! This keeps terminal sizing and snapshot acquisition out of a specific pane
//! implementation so future `heca-grid-ui` pane shells can host terminals
//! through the same contract.

use crate::actions::ActionRegistry;
use crate::app::backend_store::BackendStore;
use crate::app::interaction::{InteractionSource, dispatch_action};
use crate::app::selection_model::{SelectionOwner, SelectionRegion, SelectionSource};
use crate::app_state::{AppState, InputMode};
use heca_grid_ui::Component as _;
use heca_grid_ui::PointerButton;
use heca_grid_ui::reactive::SignalUpdate as _;

/// Default cell height in logical pixels for PixelDelta → line conversion
/// fallback when the terminal backend cannot be queried for real cell metrics.
const DEFAULT_CELL_H: f64 = 14.0;
use crate::chrome::terminal::Cell;
use crate::input::{FontZoomStep, WmAction};
use heca_core::backend::{
    BackendModifiers, BackendMouseButton, BackendMouseEvent, BackendMouseEventKind, PaneBackend,
    TerminalDamage, TerminalSnapshot,
};
use heca_core::layout::{PaneId, Rectangle};
use winit::event::MouseScrollDelta;

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

pub(crate) fn prepare_terminal_mount(
    backends: &mut BackendStore,
    pane_id: PaneId,
    content_rect: Rectangle,
    base_cell_size: (f32, f32),
    scale: f32,
) -> Option<TerminalMount> {
    let backend = backends.get_mut(pane_id)?;
    sync_terminal_backend_size(backend, content_rect, base_cell_size, scale);
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
    scale: f32,
) {
    let (approx_cell_w, approx_cell_h) = base_cell_size;
    if approx_cell_w <= 0.0 || approx_cell_h <= 0.0 {
        return;
    }

    let cols = fitted_grid_units(content_rect.size.w as f32, approx_cell_w);
    let rows = fitted_grid_units(content_rect.size.h as f32, approx_cell_h);
    let fitted_cell_w = (content_rect.size.w as f32 / cols as f32).max(1.0);
    let fitted_cell_h = (content_rect.size.h as f32 / rows as f32).max(1.0);

    // Keep the device scale current so inline images report physical pixels
    // (crisp on HiDPI). Cheap: the backend ignores an unchanged scale.
    backend.set_scale_factor(scale);
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

/// **The presses a terminal's program has heard and not yet seen released**, so a release is passed
/// on only where its press was: one that ends a divider resize, or a selection, is not the
/// program's.
#[derive(Default)]
pub(crate) struct HeardPresses(Vec<(PaneId, BackendMouseButton)>);

impl HeardPresses {
    /// The program heard this press.
    pub(crate) fn heard(&mut self, pane: PaneId, button: BackendMouseButton) {
        if !self.0.contains(&(pane, button)) {
            self.0.push((pane, button));
        }
    }

    /// A release came: was its press heard? Forgets it either way.
    pub(crate) fn released(&mut self, pane: PaneId, button: BackendMouseButton) -> bool {
        match self.0.iter().position(|p| *p == (pane, button)) {
            Some(at) => {
                self.0.swap_remove(at);
                true
            }
            None => false,
        }
    }
}

/// **A button went down on a terminal.** What it means needs state — the selection, the settings,
/// the window gesture that may have claimed the same press — so it is decided here.
///
/// The terminal does not take the press, so a window gesture (a divider, the move modifier) has
/// already decided about it by the time this runs, and what it decided is in the state.
fn on_press(
    state: &mut AppState,
    pane_id: PaneId,
    button: PointerButton,
    cell: Option<Cell>,
    modifiers: heca_grid_ui::Modifiers,
) {
    // A plain left-click while a selection is active clears it and leaves selection mode — a click
    // in a terminal puts you back in the program. Placing a caret at the click instead would trap
    // the user in selection mode until Esc (the same as most terminal emulators).
    if button == PointerButton::Left && !modifiers.shift && state.selection.is_active() {
        state.selection.clear();
        if matches!(state.input_mode, InputMode::Selection) {
            state.input_mode = InputMode::Normal;
        }
        state.needs_redraw = true;
    }
    if window_gesture_has_it(state) {
        return;
    }
    // **Host selection: Shift + left press.** It goes to the shared selection model instead of the
    // program, which keeps a program's own mouse use (a plain click) intact while giving the host an
    // explicit entry gesture.
    if button == PointerButton::Left && modifiers.shift {
        let Some(cell) = cell else {
            return;
        };
        // Only a pane whose backend has a cell grid; a future browser or GUI pane would use its
        // own selection.
        let has_grid = state
            .backends
            .get(pane_id)
            .and_then(|backend| backend.terminal_snapshot())
            .is_some();
        if has_grid {
            begin_terminal_selection_at(
                state,
                pane_id,
                cell.row,
                cell.col,
                SelectionSource::MouseDrag,
            );
        }
        return;
    }
    let Some(button) = backend_button(button) else {
        return;
    };
    let Some(cell) = cell else {
        return;
    };
    let event = backend_mouse_event(BackendMouseEventKind::Press, button, cell, modifiers);
    if let Some(backend) = state.backends.get_mut(pane_id) {
        let _ = backend.process_mouse_event(&event);
        // The release that pairs with this press is the program's to hear.
        state.terminal_presses.heard(pane_id, button);
    }
}

/// **A button came up over a terminal.** The program hears it only if it heard the press — a
/// release that ends a divider resize, or a selection, is not the program's.
fn on_release(
    state: &mut AppState,
    pane_id: PaneId,
    button: PointerButton,
    cell: Option<Cell>,
    modifiers: heca_grid_ui::Modifiers,
) {
    let Some(button) = backend_button(button) else {
        return;
    };
    if !state.terminal_presses.released(pane_id, button) {
        return;
    }
    let Some(cell) = cell else {
        return;
    };
    let event = backend_mouse_event(BackendMouseEventKind::Release, button, cell, modifiers);
    if let Some(backend) = state.backends.get_mut(pane_id) {
        let _ = backend.process_mouse_event(&event);
    }
}

/// **The pointer moved over a terminal.**
fn on_move(
    state: &mut AppState,
    pane_id: PaneId,
    cell: Option<Cell>,
    modifiers: heca_grid_ui::Modifiers,
) {
    if window_gesture_has_it(state) || crate::mouse::is_resizing(state) {
        return;
    }
    // A host selection drag follows the pointer, in the pane it started in. Over another pane, or
    // off the grid, it holds where it was — so a release outside still confirms the selection up to
    // the last cell it reached.
    if state.selection.is_selecting()
        && state.selection.source() == Some(SelectionSource::MouseDrag)
    {
        if let Some(SelectionOwner::Pane(owner)) = state.selection.owner()
            && owner == pane_id
            && let Some(cell) = cell
            && let Some(snapshot) = state
                .backends
                .get(pane_id)
                .and_then(|backend| backend.terminal_snapshot())
        {
            state
                .selection
                .update_focus(visible_row_to_stable_row(&snapshot, cell.row), cell.col);
            state.needs_redraw = true;
        }
        return;
    }
    // Only the pane that has the keyboard is told where the pointer is.
    if state.focused_pane != Some(pane_id) {
        return;
    }
    let Some(cell) = cell else {
        return;
    };
    let event = backend_mouse_event(
        BackendMouseEventKind::Move,
        BackendMouseButton::None,
        cell,
        modifiers,
    );
    if let Some(backend) = state.backends.get_mut(pane_id) {
        let _ = backend.process_mouse_event(&event);
    }
}

/// Has a window gesture taken the pointer — a move of a pane, a drag in flight, a menu or dialog
/// that just opened? Then the program does not hear it.
fn window_gesture_has_it(state: &AppState) -> bool {
    state.mouse.interactive_move.is_some()
        || crate::chrome::drag_in_flight(state)
        || crate::chrome::top_modal(state).is_some()
}

/// **The left button came up, anywhere.** A host selection drag ends where the button does, even
/// when the pointer has left the terminal it started in: the selection is confirmed up to the last
/// cell it reached and copied, as a complete gesture (select, release, copy) — the same as `y` does.
pub(crate) fn on_left_release(state: &mut AppState, registry: &ActionRegistry) {
    if state.selection.is_selecting()
        && state.selection.source() == Some(SelectionSource::MouseDrag)
    {
        state.selection.end();
        dispatch_action(
            state,
            registry,
            InteractionSource::MouseContent,
            &WmAction::CopySelection,
        );
        state.needs_redraw = true;
    }
}

fn backend_button(button: PointerButton) -> Option<BackendMouseButton> {
    match button {
        PointerButton::Left => Some(BackendMouseButton::Left),
        PointerButton::Middle => Some(BackendMouseButton::Middle),
        PointerButton::Right => Some(BackendMouseButton::Right),
        PointerButton::Other(_) => None,
    }
}

/// **A terminal said something about its input.** The terminal worked out where in its grid the
/// pointer was; what to *do* about it needs state — the process, the mouse-grab, the settings — so
/// it is decided here, in one place.
pub(crate) fn on_terminal_input(
    state: &mut AppState,
    registry: &ActionRegistry,
    pane_id: PaneId,
    input: crate::chrome::terminal::TerminalInput,
) {
    use crate::chrome::terminal::TerminalInput;
    match input {
        TerminalInput::Wheel {
            x,
            y,
            pixels,
            cell,
            modifiers,
        } => on_wheel(state, registry, pane_id, ((x, y), pixels), cell, modifiers),
        TerminalInput::Press {
            button,
            cell,
            modifiers,
        } => on_press(state, pane_id, button, cell, modifiers),
        TerminalInput::Release {
            button,
            cell,
            modifiers,
        } => on_release(state, pane_id, button, cell, modifiers),
        TerminalInput::Move { cell, modifiers } => on_move(state, pane_id, cell, modifiers),
    }
}

/// **The wheel as the device reported it**, which is what the scrollback policy is written in: a
/// device that counts pixels (a trackpad) is scrolled by the cell height, one that counts notches by
/// notches. The grid's deltas run the other way round from the device's, so both are negated back.
fn device_delta((x, y): (f32, f32), pixels: Option<(f32, f32)>) -> MouseScrollDelta {
    match pixels {
        Some((px, py)) => MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(
            -(px as f64),
            -(py as f64),
        )),
        None => MouseScrollDelta::LineDelta(-x, -y),
    }
}

fn on_wheel(
    state: &mut AppState,
    registry: &ActionRegistry,
    pane_id: PaneId,
    ((x, y), pixels): ((f32, f32), Option<(f32, f32)>),
    cell: Option<Cell>,
    modifiers: heca_grid_ui::Modifiers,
) {
    // **Ctrl/Meta+wheel is a font-zoom gesture**, resolved by the terminal it turned over. It is
    // consumed whatever the direction, so a modified wheel never reaches the program as a scroll.
    if state.mouse_wheel_change_font_size && (modifiers.ctrl || modifiers.meta) {
        if y != 0.0 {
            let step = if y < 0.0 {
                FontZoomStep::In
            } else {
                FontZoomStep::Out
            };
            dispatch_action(
                state,
                registry,
                InteractionSource::MouseContent,
                &WmAction::PaneTerminalFontZoom {
                    pane_id: Some(pane_id),
                    step,
                },
            );
        }
        return;
    }
    if state.mouse.interactive_move.is_some() || crate::chrome::drag_in_flight(state) {
        return;
    }
    // The grid's lines run the other way round from the device's: positive `y` is content moving
    // down, which is the wheel turned toward you.
    //
    // A device that counts pixels (a trackpad) is scrolled by the cell height, as it always was;
    // one that counts notches, by notches.
    let delta = device_delta((x, y), pixels);

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
    // The wheel only scrolls: it never switches to selection mode (Antonio, 2026-09-29 — a user who
    // drives heca without the prefix would be left in a mode they never asked for). Typing snaps
    // the view back to the bottom; selection mode is entered with `enter_selection_mode`.
    let shift_held = modifiers.shift;
    let wants_mouse = state
        .backends
        .get(pane_id)
        .is_some_and(|b| b.is_mouse_grabbed());
    let do_host_scroll = shift_held || (state.terminal_mouse_enabled && !wants_mouse);

    if !do_host_scroll {
        // Forward wheel to the terminal backend.
        forward_wheel_to_terminal(state, pane_id, cell, modifiers, delta);
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
        .get(pane_id)
        .and_then(|b| b.terminal_snapshot())
        .is_some_and(|s| s.scrollback_rows > s.rows);
    if !host_can_scroll {
        if shift_held {
            // Documented limitation: Shift+wheel host scrollback is a no-op
            // while the backend is in an alternate screen (no exposed host
            // history).
            return;
        }
        forward_wheel_to_terminal(state, pane_id, cell, modifiers, delta);
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
            .get(pane_id)
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

    if let Some(backend) = state.backends.get_mut(pane_id) {
        backend.scroll_viewport(delta_i32);
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
    ev: &heca_grid_ui::Event,
) -> bool {
    use heca_grid_ui::event::RawPointerKind as Kind;
    let heca_grid_ui::Event::Raw(raw) = ev else {
        return false;
    };
    if raw.button != heca_grid_ui::PointerButton::Left
        || raw.kind != Kind::Pressed
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
    let Some(pane_id) = crate::mouse::hit_test_pane(state, pos) else {
        return false;
    };
    state
        .backends
        .get(pane_id)
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

/// **The backend's mouse event for a pointer at `cell`.** Everything it needs is in the arguments:
/// the terminal worked out the cell, and the modifiers rode in with the event.
fn backend_mouse_event(
    kind: BackendMouseEventKind,
    button: BackendMouseButton,
    cell: Cell,
    modifiers: heca_grid_ui::Modifiers,
) -> BackendMouseEvent {
    BackendMouseEvent {
        kind,
        col: cell.col,
        row: cell.row,
        x_pixel_offset: cell.x_offset,
        y_pixel_offset: cell.y_offset,
        button,
        modifiers: BackendModifiers {
            ctrl: modifiers.ctrl,
            shift: modifiers.shift,
            alt: modifiers.alt,
            super_: modifiers.meta,
        },
    }
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
    // A mouse selection is a gesture, not a mode (Antonio, 2026-09-29): Shift+drag highlights,
    // release copies, and the keyboard stays where it was. Selection mode is the keyboard's.
    state.needs_redraw = true;
}

fn visible_row_to_stable_row(snapshot: &heca_core::backend::TerminalSnapshot, row: usize) -> isize {
    snapshot.viewport_top_stable_row + row as isize
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
    Cell::at((pos.0 as f64, pos.1 as f64), content_rect, cell_w, cell_h).map(|c| (c.row, c.col))
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
    let window = crate::chrome::ChromeConfig::of(state).window();
    let (win_w, win_h) = (window.w as f32, window.h as f32);
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

/// Enter scrollback-search query entry for the selection's (or focused) pane.
/// No-op when there is no terminal-backed pane to search. terminal-task-19.
pub(crate) fn enter_scrollback_search(state: &mut AppState) {
    let Some(pane_id) = state.search_target_pane() else {
        return;
    };
    if state
        .backends
        .get(pane_id)
        .and_then(|b| b.terminal_snapshot())
        .is_none()
    {
        return;
    }
    // Re-entering search on a pane that already has one RESUMES it: the query, its
    // matches and the caret are all still there. Inserting a fresh state here wiped
    // whatever had been typed, so `/` after Enter was indistinguishable from having
    // no way back into the field at all.
    if let Some(existing) = state.searches.get(&pane_id) {
        existing.input.borrow_mut().base_mut().focused.set(true);
        state.input_mode = InputMode::Search;
        state.needs_redraw = true;
        return;
    }
    state.searches.insert(
        pane_id,
        crate::app_state::SearchState {
            // Focused so the caret shows and the field accepts editing keys.
            input: std::cell::RefCell::new({
                let mut field = heca_grid_ui::widgets::Input::new();
                field.base_mut().focused.set(true);
                field
            }),
            matches: Vec::new(),
            current: None,
        },
    );
    state.input_mode = InputMode::Search;
    state.needs_redraw = true;
}

/// Re-run the search for the current query, refresh the match list, focus the match
/// nearest at/above the caret (else the last), and jump to it.
pub(crate) fn run_scrollback_search(state: &mut AppState) {
    // The pane whose query the keyboard is editing — same resolution the search was
    // started with, so edits land on the entry that exists.
    let Some(pane_id) = state.search_target_pane() else {
        return;
    };
    let Some(query) = state
        .search_for(pane_id)
        .map(|s| s.input.borrow().value_str())
    else {
        return;
    };
    let cols = match state
        .backends
        .get(pane_id)
        .and_then(|b| b.terminal_snapshot())
    {
        Some(snap) => snap.cols,
        None => return,
    };
    let caret_row = state.selection.cursor_cell().map(|(_, row, _)| row);
    let matches = state
        .backends
        .get(pane_id)
        .map(|b| b.search_scrollback(&query, cols))
        .unwrap_or_default();
    let current = if matches.is_empty() {
        None
    } else {
        let caret = caret_row.unwrap_or(isize::MAX);
        // Nearest match at/above the caret, else fall back to the last match.
        Some(
            matches
                .iter()
                .rposition(|m| m.stable_row <= caret)
                .unwrap_or(matches.len() - 1),
        )
    };
    if let Some(search) = state.searches.get_mut(&pane_id) {
        search.matches = matches;
        search.current = current;
    }
    jump_to_current_match(state);
}

/// Move the focused match by one (wrapping) and jump to it. `forward` = next match.
pub(crate) fn search_step(state: &mut AppState, forward: bool) {
    let stepped = state.active_search_mut().and_then(|search| {
        let n = search.matches.len();
        if n == 0 {
            return None;
        }
        let cur = search.current.unwrap_or(0);
        let next = if forward {
            (cur + 1) % n
        } else {
            (cur + n - 1) % n
        };
        search.current = Some(next);
        Some(())
    });
    if stepped.is_some() {
        jump_to_current_match(state);
    }
}

/// Place the selection caret on the focused match's first cell and scroll it into
/// view. No-op when no match is focused.
fn jump_to_current_match(state: &mut AppState) {
    let Some(pane_id) = state.search_target_pane() else {
        return;
    };
    let Some(m) = state
        .search_for(pane_id)
        .and_then(|s| s.current.and_then(|i| s.matches.get(i)).cloned())
    else {
        return;
    };
    state
        .selection
        .set_caret(SelectionOwner::Pane(pane_id), m.stable_row, m.start_col);
    if let Some(snapshot) = state
        .backends
        .get(pane_id)
        .and_then(|b| b.terminal_snapshot())
    {
        ensure_caret_visible(state, pane_id, m.stable_row, &snapshot);
    }
    state.needs_redraw = true;
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
    pane_id: PaneId,
    cell: Option<Cell>,
    modifiers: heca_grid_ui::Modifiers,
    delta: MouseScrollDelta,
) {
    let Some(cell) = cell else {
        return;
    };
    for button in wheel_buttons(delta) {
        let event = backend_mouse_event(BackendMouseEventKind::Press, button, cell, modifiers);
        if let Some(backend) = state.backends.get_mut(pane_id) {
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

/// The **outer** screen rect (full pane, before content inset) of every visible
/// pane — tiled then floating — in the active workspace. Same geometry the render
/// loop derives per pane (`render.rs`); kept here so the pane-header sync step can
/// position the in-pane info bar without a GPU borrow (render's `scene_view` holds
/// `state.compositor`). Returns `(pane_id, x, y, w, h)` in logical px.
/// **The columns of the active workspace, in screen coordinates**, with the panes inside each.
///
/// The same geometry [`pane_outer_frames`] reports, grouped the way the tree is shaped. Built on
/// `ScrollingSpace::columns_with_positions`, so a caller asking about a column and a caller asking
/// about a pane read one walk.
///
/// Floating panes are **not** here: they belong to no column.
pub(crate) fn column_frames(state: &AppState) -> Vec<heca_core::layout::LaidOutColumn> {
    let pane_area = crate::chrome::ChromeConfig::of(state).content_rect();
    let ws_offset = state
        .session
        .workspace_geometries()
        .first()
        .map(|(_, rect)| (rect.loc.x, rect.loc.y))
        .unwrap_or((0.0, 0.0));
    let (dx, dy) = (pane_area.loc.x + ws_offset.0, pane_area.loc.y + ws_offset.1);
    let Some(ws) = state.session.active_workspace() else {
        return Vec::new();
    };
    let shift = |r: heca_core::layout::Rectangle| {
        heca_core::layout::Rectangle::new(
            heca_core::layout::types::Point::new(r.loc.x + dx, r.loc.y + dy),
            r.size,
        )
    };
    ws.scrolling
        .columns_with_positions()
        .into_iter()
        .map(|mut col| {
            col.rect = shift(col.rect);
            for pane in &mut col.panes {
                pane.rect = shift(pane.rect);
                pane.slot = shift(pane.slot);
            }
            col
        })
        .collect()
}

pub(crate) fn pane_outer_frames(state: &AppState) -> Vec<(PaneId, f32, f32, f32, f32)> {
    let pane_area = crate::chrome::ChromeConfig::of(state).content_rect();
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

/// **The box a pane's terminal fills**, as the last frame drew it — the one the pointer is mapped
/// against, so what is clicked is what was drawn.
fn content_rect_for_pane(state: &AppState, pane_id: PaneId) -> Option<Rectangle> {
    let (rect, drawn) = crate::chrome::terminal::content_box(state, pane_id);
    rect.filter(|_| drawn)
}

#[cfg(test)]
mod tests {
    use super::{HeardPresses, device_delta, host_scroll_notches, hyperlink_at_cell};
    use heca_core::backend::BackendMouseButton;
    use heca_core::backend::HyperlinkSpan;
    use heca_core::layout::PaneId;
    use winit::event::MouseScrollDelta;

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
        assert_eq!(hyperlink_at_cell(&links, 2, 4), Some("https://example.com"));
        // Interior cell.
        assert_eq!(hyperlink_at_cell(&links, 2, 8), Some("https://example.com"));
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

    #[test]
    fn a_trackpad_keeps_its_pixels_and_a_wheel_its_notches() {
        // Content moving down (+y) is the device turned toward you (-y).
        assert!(matches!(
            device_delta((0.0, 3.0), None),
            MouseScrollDelta::LineDelta(_, y) if y == -3.0
        ));
        assert!(matches!(
            device_delta((0.0, 0.7), Some((0.0, 14.0))),
            MouseScrollDelta::PixelDelta(p) if p.y == -14.0
        ));
        // And the pixels, not the lines, decide the unit: 14 px is one 14-px cell.
        assert_eq!(
            host_scroll_notches(device_delta((0.0, 0.7), Some((0.0, 14.0))), 14.0),
            -1.0
        );
    }

    #[test]
    fn a_release_is_the_programs_only_where_its_press_was() {
        let mut heard = HeardPresses::default();
        let (a, b) = (PaneId(1), PaneId(2));
        heard.heard(a, BackendMouseButton::Left);
        assert!(
            !heard.released(b, BackendMouseButton::Left),
            "another pane's"
        );
        assert!(
            !heard.released(a, BackendMouseButton::Right),
            "another button's"
        );
        assert!(heard.released(a, BackendMouseButton::Left));
        assert!(
            !heard.released(a, BackendMouseButton::Left),
            "heard once, released once"
        );
    }
}
