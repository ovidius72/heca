//! Terminal host adapter between pane shells and terminal backends.
//!
//! This keeps terminal sizing and snapshot acquisition out of a specific pane
//! implementation so future `heca-grid-ui` pane shells can host terminals
//! through the same contract.

use crate::app::backend_store::BackendStore;
use crate::app_state::{AppState, InteractiveMovePhase};
use heca_core::backend::{
    BackendModifiers, BackendMouseButton, BackendMouseEvent, BackendMouseEventKind, PaneBackend,
    TerminalSnapshot,
};
use heca_core::layout::{PaneId, Point, Rectangle, Size};
use winit::event::{ElementState, MouseButton, MouseScrollDelta};

pub(crate) struct TerminalMount {
    pub content_rect: Rectangle,
    pub snapshot: TerminalSnapshot,
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
    let _ = backend.take_terminal_damage();

    Some(TerminalMount {
        content_rect,
        snapshot,
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

    let Some(target) = terminal_target_at_position(state, pos) else {
        return;
    };
    if state.focused_pane != Some(target.pane_id) {
        return;
    }

    let Some(event) = build_mouse_event(state, target, pos, BackendMouseEventKind::Move, BackendMouseButton::None) else {
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
) {
    if started_interactive_move(state, button, button_state) || state.mouse.drag_ctx.is_dragging() {
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

pub(crate) fn forward_mouse_wheel(
    state: &mut AppState,
    pos: (f32, f32),
    delta: MouseScrollDelta,
) {
    if state.mouse.interactive_move.is_some() || state.mouse.drag_ctx.is_dragging() {
        return;
    }

    let Some(target) = terminal_target_at_position(state, pos) else {
        return;
    };
    for button in wheel_buttons(delta) {
        let Some(event) = build_mouse_event(state, target, pos, BackendMouseEventKind::Press, button) else {
            continue;
        };
        if let Some(backend) = state.backends.get_mut(target.pane_id) {
            let _ = backend.process_mouse_event(&event);
        }
    }
}

pub(crate) fn notify_focus_changed(state: &mut AppState, prev: Option<PaneId>, next: Option<PaneId>) {
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

fn build_mouse_event(
    state: &AppState,
    target: TerminalInputTarget,
    pos: (f32, f32),
    kind: BackendMouseEventKind,
    button: BackendMouseButton,
) -> Option<BackendMouseEvent> {
    let local_x = pos.0 as f64 - target.content_rect.loc.x;
    let local_y = pos.1 as f64 - target.content_rect.loc.y;
    if local_x < 0.0
        || local_y < 0.0
        || local_x >= target.content_rect.size.w
        || local_y >= target.content_rect.size.h
    {
        return None;
    }

    let (cell_w, cell_h) = state
        .backends
        .get(target.pane_id)
        .map(|backend| backend.cell_size())
        .unwrap_or(state.terminal_cell_size);
    if cell_w <= 0.0 || cell_h <= 0.0 {
        return None;
    }

    let col = (local_x / cell_w as f64).floor().max(0.0) as usize;
    let row = (local_y / cell_h as f64).floor().max(0.0) as usize;
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

fn axis_wheel_buttons(x: f64, y: f64) -> Vec<BackendMouseButton> {
    let mut buttons = Vec::new();
    push_wheel_buttons(&mut buttons, y, BackendMouseButton::WheelUp, BackendMouseButton::WheelDown);
    push_wheel_buttons(&mut buttons, x, BackendMouseButton::WheelRight, BackendMouseButton::WheelLeft);
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
            let border = state.theme.border_width * 2.0;
            return inset_content_rect(x, y, w, h, border);
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
        let border = state.theme.border_width;
        return inset_content_rect(x, y, w, h, border);
    }

    None
}

fn inset_content_rect(x: f32, y: f32, w: f32, h: f32, border_width: f32) -> Option<Rectangle> {
    let inset = border_width.max(1.0);
    let content_w = (w - inset * 2.0).max(0.0);
    let content_h = (h - inset * 2.0).max(0.0);
    if content_w <= 0.0 || content_h <= 0.0 {
        return None;
    }

    Some(Rectangle::new(
        Point::new((x + inset) as f64, (y + inset) as f64),
        Size::new(content_w as f64, content_h as f64),
    ))
}

fn chrome_config(state: &AppState) -> crate::chrome::ChromeConfig {
    crate::chrome::ChromeConfig {
        tab_bar_height: crate::chrome::DEFAULT_TAB_BAR_HEIGHT,
        status_bar_height: crate::chrome::DEFAULT_STATUS_BAR_HEIGHT,
        left_sidebar_width: if state.sidebar.left_visible {
            state.sidebar.left_width
        } else {
            crate::chrome::DEFAULT_COLLAPSED_SIDEBAR_WIDTH
        },
        right_sidebar_width: if state.sidebar.right_visible {
            state.sidebar.right_width
        } else {
            crate::chrome::DEFAULT_COLLAPSED_SIDEBAR_WIDTH
        },
    }
}

fn window_logical_size(state: &AppState) -> (f32, f32) {
    let phys = state.window.inner_size();
    (
        phys.width as f32 / state.scale_factor as f32,
        phys.height as f32 / state.scale_factor as f32,
    )
}
