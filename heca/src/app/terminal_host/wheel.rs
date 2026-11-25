//! **What the wheel means over a terminal** — font zoom, host scrollback, or the wheel handed to the
//! program, decided from the mouse-grab, the settings and the device that turned it.
//!
//! **One measure of a turn, for both places it goes.** A turn becomes whole *notches* — a wheel's
//! clicks, or a trackpad's pixels counted in cells, with what is left over kept for the next turn
//! ([`WheelRemainders`]) — and each notch is `terminal_wheel_scroll_lines` lines, whether the host
//! scrolls its history by them or the program (Claude Code, vim, less) is told of them, one wheel
//! report per line. Two calculations of one gesture is how the setting came to change the shell's
//! history and do nothing inside a program.

use crate::actions::ActionRegistry;
use crate::app::interaction::{InteractionSource, dispatch_action};
use crate::app_state::AppState;
use crate::chrome::panes::terminal::Cell;
use crate::chrome::panes::terminal::TerminalId;
use crate::input::{FontZoomStep, WmAction};
use heca_core::backend::{BackendMouseButton, BackendMouseEventKind};
use std::collections::HashMap;
use winit::event::MouseScrollDelta;

use super::input::backend_mouse_event;

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

/// What a wheel turn means for a terminal's font.
#[derive(Debug, PartialEq)]
enum WheelZoom {
    /// Not a zoom gesture: the setting is off, or neither Ctrl nor Meta is held.
    Not,
    /// A zoom gesture that steps nothing (no vertical turn), consumed all the same.
    Consumed,
    /// The zoom of the terminal it turned over.
    Step(WmAction),
}

/// **What a wheel turn means for the font of `terminal`** — turning away from you makes it
/// bigger.
fn wheel_font_zoom(
    enabled: bool,
    modifiers: heca_grid_ui::Modifiers,
    y: f32,
    terminal: TerminalId,
) -> WheelZoom {
    if !enabled || !(modifiers.ctrl || modifiers.meta) {
        return WheelZoom::Not;
    }
    if y == 0.0 {
        return WheelZoom::Consumed;
    }
    let step = if y < 0.0 {
        FontZoomStep::In
    } else {
        FontZoomStep::Out
    };
    WheelZoom::Step(WmAction::PaneTerminalFontZoom {
        pane_id: None,
        terminal: Some(terminal.0),
        step,
    })
}

pub(super) fn on_wheel(
    state: &mut AppState,
    registry: &ActionRegistry,
    terminal: TerminalId,
    ((x, y), pixels): ((f32, f32), Option<(f32, f32)>),
    cell: Option<Cell>,
    modifiers: heca_grid_ui::Modifiers,
) {
    // **Ctrl/Meta+wheel is a font-zoom gesture** for the terminal it turned over, whoever owns it.
    // It is consumed whatever the direction, so a modified wheel never reaches the program as a
    // scroll.
    match wheel_font_zoom(
        state.settings.mouse_wheel_change_font_size,
        modifiers,
        y,
        terminal,
    ) {
        WheelZoom::Not => {}
        WheelZoom::Consumed => return,
        WheelZoom::Step(action) => {
            let _ = dispatch_action(state, registry, InteractionSource::MouseContent, &action);
            return;
        }
    }
    if crate::chrome::drag_in_flight(state) {
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
    // The wheel only scrolls: it never switches to selection mode. Typing snaps the view back to
    // the bottom; selection mode is entered with `enter_selection_mode`.
    let shift_held = modifiers.shift;
    let wants_mouse = state
        .client
        .backends
        .get_by_id(terminal)
        .is_some_and(|b| b.is_mouse_grabbed());
    let do_host_scroll = shift_held || (state.settings.terminal_mouse_enabled && !wants_mouse);

    if !do_host_scroll {
        // Forward wheel to the terminal backend.
        forward_wheel_to_terminal(state, terminal, cell, modifiers, delta);
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
    // alt-screen TUIs is an inherent limitation (see docs/keyboard-commands.md).
    let host_can_scroll = state
        .client
        .backends
        .get_by_id(terminal)
        .and_then(|b| b.terminal_snapshot())
        .is_some_and(|s| s.scrollback_rows > s.rows);
    if !host_can_scroll {
        if shift_held {
            // Documented limitation: Shift+wheel host scrollback is a no-op
            // while the backend is in an alternate screen (no exposed host
            // history).
            return;
        }
        forward_wheel_to_terminal(state, terminal, cell, modifiers, delta);
        return;
    }

    // Host scrollback is vertical. On many platforms Shift+wheel is remapped to a horizontal turn
    // (`x`) with `y == 0`; that gesture still means "scroll the history", so a turn with no vertical
    // part is read from `x`.
    let (lx, ly) = lines_of(state, terminal, delta);
    let rows = if ly != 0 { ly } else { lx };
    if rows == 0 {
        return;
    }
    if let Some(backend) = state.client.backends.get_mut_by_id(terminal) {
        backend.scroll_viewport(rows);
    }
    state.needs_redraw = true;
    // Reset prefix timeout on scroll (like keyboard input).
    state.prefix_entered_at = None;
}

/// **Wheel turns not yet worth a whole line**, per terminal and axis — what makes a slow turn scroll
/// as far as a fast one of the same length, instead of every tiny movement counting (or none of them
/// counting at all).
///
/// It is counted in **lines**, after `terminal_wheel_scroll_lines` is applied — never in notches
/// first. A wheel turned slowly on macOS reports a tenth of a notch at a time; counted in notches it
/// took ten turns to make one, which then jumped every line of it at once and felt like nothing
/// scrolled at all.
#[derive(Default)]
pub(crate) struct WheelRemainders(HashMap<TerminalId, (f64, f64)>);

impl WheelRemainders {
    /// **The whole lines `(x, y)` this turn completes**, keeping the rest for the next. A notch is
    /// `per_notch` lines; a trackpad's pixels are counted in cells (`cell` = width, height), a cell
    /// being a notch.
    ///
    /// **The size of a wheel's turn is the system's, and it is kept.** macOS already reports a
    /// wheel turned slowly as a tenth of a notch and one turned fast as five to fifteen, so the
    /// lines follow the speed of the hand. A click too small to make a line still scrolls **one
    /// line** — never a whole notch, which made the gentlest click jump `per_notch` lines and left
    /// no way to move by one.
    fn lines(
        &mut self,
        terminal: TerminalId,
        delta: MouseScrollDelta,
        cell: (f64, f64),
        per_notch: f64,
    ) -> (i32, i32) {
        let (wheel, (nx, ny)) = match delta {
            MouseScrollDelta::LineDelta(x, y) => (true, (f64::from(x), f64::from(y))),
            MouseScrollDelta::PixelDelta(p) => {
                (false, (per_cell(p.x, cell.0), per_cell(p.y, cell.1)))
            }
        };
        let (dx, dy) = (nx * per_notch, ny * per_notch);
        let left = self.0.entry(terminal).or_default();
        let (x, y) = (
            whole_lines(&mut left.0, dx, wheel),
            whole_lines(&mut left.1, dy, wheel),
        );
        (x, y)
    }
}

/// The whole lines `lines` more makes on one axis, with what is `left` from before; the rest is
/// kept in `left`. A wheel's click (`wheel`) that makes no line by itself still makes one, and
/// starts the count again — a slow click scrolls every time, a trackpad adds up.
fn whole_lines(left: &mut f64, lines: f64, wheel: bool) -> i32 {
    let sum = *left + lines;
    let whole = sum.trunc();
    if whole == 0.0 && wheel && lines != 0.0 {
        *left = 0.0;
        return lines.signum() as i32;
    }
    *left = sum - whole;
    whole as i32
}

/// Pixels counted in cells — none when the cell size is not known yet.
fn per_cell(pixels: f64, cell: f64) -> f64 {
    if cell > 0.0 { pixels / cell } else { 0.0 }
}

/// **The whole lines a turn over `terminal` completes**, measured in that terminal's cells.
fn lines_of(state: &mut AppState, terminal: TerminalId, delta: MouseScrollDelta) -> (i32, i32) {
    let per_notch = f64::from(lines_per_notch(state));
    let cell = state
        .client
        .backends
        .get_by_id(terminal)
        .map(|b| b.cell_size())
        .unwrap_or(state.terminal_cell_size);
    state.terminal_wheel.lines(
        terminal,
        delta,
        (f64::from(cell.0), f64::from(cell.1)),
        per_notch,
    )
}

/// Lines per notch — `terminal_wheel_scroll_lines`, for the history and the program alike.
fn lines_per_notch(state: &AppState) -> i32 {
    i32::try_from(state.settings.terminal_wheel_scroll_lines).unwrap_or(i32::MAX)
}

/// **The wheel reports a program is sent for `(x, y)` lines** — one report per line, so a notch is
/// as many lines inside Claude Code or vim as it is in the shell's history. The program's terminal
/// writes one report per event whatever count it is given, which is why each line is its own event.
fn wheel_reports((x, y): (i32, i32)) -> Vec<BackendMouseButton> {
    let mut reports = Vec::new();
    let mut push = |lines: i32, positive: BackendMouseButton, negative: BackendMouseButton| {
        let button = if lines > 0 { positive } else { negative };
        reports.extend(std::iter::repeat_n(button, lines.unsigned_abs() as usize));
    };
    push(
        y,
        BackendMouseButton::WheelUp(1),
        BackendMouseButton::WheelDown(1),
    );
    push(
        x,
        BackendMouseButton::WheelRight(1),
        BackendMouseButton::WheelLeft(1),
    );
    reports
}

/// Forward a wheel scroll to the terminal backend as press events.
///
/// Used by [`on_wheel`] both when the policy routes the wheel away
/// from host scrollback (grabbed TUI / `terminal_mouse = false`) and as a
/// graceful fallback when the host viewport has no scrollback room (see
/// `terminal-task-01h`).
fn forward_wheel_to_terminal(
    state: &mut AppState,
    terminal: TerminalId,
    cell: Option<Cell>,
    modifiers: heca_grid_ui::Modifiers,
    delta: MouseScrollDelta,
) {
    let Some(cell) = cell else {
        return;
    };
    for button in wheel_reports(lines_of(state, terminal, delta)) {
        let event = backend_mouse_event(BackendMouseEventKind::Press, button, cell, modifiers);
        if let Some(backend) = state.client.backends.get_mut_by_id(terminal) {
            let _ = backend.process_mouse_event(&event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut left = WheelRemainders::default();
        assert_eq!(
            left.lines(
                TerminalId(1),
                device_delta((0.0, 0.7), Some((0.0, 14.0))),
                (8.0, 14.0),
                1.0
            ),
            (0, -1)
        );
    }

    /// **A slow trackpad gesture adds up**: turns smaller than a cell scroll nothing until they make
    /// a whole one, and what is left over is kept — per terminal, so two terminals never share it.
    #[test]
    fn a_trackpad_turn_smaller_than_a_cell_adds_up_to_a_line() {
        let mut left = WheelRemainders::default();
        let (a, b) = (TerminalId(1), TerminalId(2));
        let turn = |px: f32| device_delta((0.0, 0.0), Some((0.0, px)));
        assert_eq!(
            left.lines(a, turn(5.0), (8.0, 14.0), 1.0),
            (0, 0),
            "under a cell: nothing yet"
        );
        assert_eq!(
            left.lines(b, turn(5.0), (8.0, 14.0), 1.0),
            (0, 0),
            "another terminal starts from zero"
        );
        assert_eq!(left.lines(a, turn(5.0), (8.0, 14.0), 1.0), (0, 0));
        assert_eq!(
            left.lines(a, turn(5.0), (8.0, 14.0), 1.0),
            (0, -1),
            "15 px make one 14-px cell"
        );
        assert_eq!(
            left.lines(a, turn(13.0), (8.0, 14.0), 1.0),
            (0, -1),
            "the 1 px left over counts"
        );
        assert_eq!(
            left.lines(a, device_delta((0.0, 2.0), None), (8.0, 14.0), 1.0),
            (0, -2),
            "a wheel's notches are whole"
        );
    }

    /// **A program hears as many lines per notch as the history scrolls**: notches × the setting,
    /// one report per line, because its terminal writes one report per event whatever count it is
    /// given.
    #[test]
    fn a_program_is_sent_one_report_per_line_of_every_notch() {
        use BackendMouseButton::{WheelDown, WheelLeft, WheelRight, WheelUp};
        assert_eq!(
            wheel_reports((0, 6)),
            vec![WheelUp(1); 6],
            "six lines, six reports"
        );
        assert_eq!(wheel_reports((0, -6)), vec![WheelDown(1); 6]);
        assert_eq!(wheel_reports((2, 0)), vec![WheelRight(1); 2]);
        assert_eq!(wheel_reports((-1, 1)), vec![WheelUp(1), WheelLeft(1)]);
        assert!(wheel_reports((0, 0)).is_empty(), "no line, no report");
        // Two notches of three lines each, counted the way a turn is.
        let mut left = WheelRemainders::default();
        assert_eq!(
            wheel_reports(left.lines(
                TerminalId(1),
                device_delta((0.0, 2.0), None),
                (8.0, 14.0),
                3.0
            )),
            vec![WheelDown(1); 6]
        );
    }

    /// **Every click of a wheel scrolls, and by as much as it was turned** — macOS reports a slow
    /// click as a tenth of a notch: it scrolls one line, never a whole notch, so a line at a time
    /// is possible. A fast turn keeps its size × `per_notch`. A trackpad, which counts pixels,
    /// still moves by the part of a cell it travelled.
    #[test]
    fn every_slow_click_of_a_wheel_scrolls_one_line() {
        let mut left = WheelRemainders::default();
        let slow_click = || device_delta((0.0, 0.1), None);
        assert_eq!(
            left.lines(TerminalId(1), slow_click(), (8.0, 14.0), 1.0),
            (0, -1)
        );
        assert_eq!(
            left.lines(TerminalId(1), slow_click(), (8.0, 14.0), 1.0),
            (0, -1)
        );
        assert_eq!(
            left.lines(TerminalId(1), slow_click(), (8.0, 14.0), 6.0),
            (0, -1),
            "a slow click is one line, not a notch of `terminal_wheel_scroll_lines`"
        );
        assert_eq!(
            left.lines(
                TerminalId(4),
                device_delta((0.0, 5.0), None),
                (8.0, 14.0),
                6.0
            ),
            (0, -30),
            "a fast turn of five is five notches"
        );
        let fast = device_delta((0.0, 2.5), None);
        assert_eq!(
            left.lines(TerminalId(2), fast, (8.0, 14.0), 1.0),
            (0, -2),
            "a fast turn keeps its size"
        );
        let little_swipe = || device_delta((0.0, 0.0), Some((0.0, 7.0)));
        assert_eq!(
            left.lines(TerminalId(3), little_swipe(), (8.0, 14.0), 1.0),
            (0, 0),
            "half a cell: not yet"
        );
        assert_eq!(
            left.lines(TerminalId(3), little_swipe(), (8.0, 14.0), 1.0),
            (0, -1)
        );
    }

    /// **A modified wheel zooms the terminal it turned over**, whoever owns it — a docked one as
    /// much as a pane's — and names that terminal, so the zoom is that terminal's alone.
    #[test]
    fn a_modified_wheel_zooms_the_terminal_it_turned_over() {
        let ctrl = heca_grid_ui::Modifiers {
            ctrl: true,
            ..Default::default()
        };
        let unowned = TerminalId(7);
        assert_eq!(
            wheel_font_zoom(true, ctrl, -1.0, unowned),
            WheelZoom::Step(WmAction::PaneTerminalFontZoom {
                pane_id: None,
                terminal: Some(7),
                step: FontZoomStep::In,
            })
        );
        let meta = heca_grid_ui::Modifiers {
            meta: true,
            ..Default::default()
        };
        assert!(matches!(
            wheel_font_zoom(true, meta, 2.0, unowned),
            WheelZoom::Step(WmAction::PaneTerminalFontZoom {
                step: FontZoomStep::Out,
                ..
            })
        ));
        assert_eq!(
            wheel_font_zoom(true, ctrl, 0.0, unowned),
            WheelZoom::Consumed,
            "a sideways turn is consumed and steps nothing"
        );
        assert_eq!(
            wheel_font_zoom(true, heca_grid_ui::Modifiers::default(), -1.0, unowned),
            WheelZoom::Not,
            "a plain wheel is a scroll"
        );
        assert_eq!(
            wheel_font_zoom(false, ctrl, -1.0, unowned),
            WheelZoom::Not,
            "the setting turns the gesture off"
        );
    }
}
