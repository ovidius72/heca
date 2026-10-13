//! **Terminal actions** — typing a line into a running terminal, ending one, and stepping one's own
//! font size. The ways anything outside the terminal (a key, the palette, RPC, heca-pro) reaches a
//! terminal process, all naming their target by one rule.

use crate::app_state::AppState;
use crate::chrome::terminal::TerminalId;
use crate::input::WmAction;
use heca_core::layout::PaneId;

/// **Which terminal an action means** — a pane's, or one no pane owns (an extension's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Pane(PaneId),
    Unowned(TerminalId),
}

/// **Which terminal an action means.** A `terminal` id wins (it is what a script that started one
/// holds), then an explicit `pane_id`, then the pane that has the keyboard. One rule, so every
/// terminal action reads its target the same way.
fn target(
    pane_id: Option<PaneId>,
    terminal: Option<u64>,
    focused: Option<PaneId>,
    find: impl Fn(TerminalId) -> Option<Target>,
) -> Option<Target> {
    match terminal {
        Some(id) => find(TerminalId(id)),
        None => pane_id.or(focused).map(Target::Pane),
    }
}

fn target_of(state: &AppState, pane_id: Option<PaneId>, terminal: Option<u64>) -> Option<Target> {
    target(pane_id, terminal, state.focused_pane, |id| {
        match state.server.backends.pane_of(id) {
            Some(pane) => Some(Target::Pane(pane)),
            None => state.server.backends.get_by_id(id).map(|_| Target::Unowned(id)),
        }
    })
}

impl Target {
    /// The terminal itself, whoever owns it.
    fn terminal(self, state: &AppState) -> Option<TerminalId> {
        match self {
            Target::Pane(pane) => state.server.backends.identity_of(pane),
            Target::Unowned(id) => Some(id),
        }
    }
}

/// **Step one terminal's own font** — bigger, smaller, or back to following the app.
///
/// A terminal or pane the action names is zoomed. Naming neither means the terminal the keyboard
/// is on: one placed in a dock or an overlay that holds it is asked first and answers itself (it
/// says so back with its id, which lands here again); otherwise the focused pane's.
pub fn handle_pane_terminal_font_zoom(state: &mut AppState, action: &WmAction) {
    let WmAction::PaneTerminalFontZoom {
        pane_id,
        terminal,
        step,
    } = action
    else {
        return;
    };
    let named = pane_id.is_some() || terminal.is_some();
    if !named && crate::app::terminal_host::tree_answers(state, step.intent()) {
        return;
    }
    let Some(id) = target_of(state, *pane_id, *terminal).and_then(|t| t.terminal(state)) else {
        return;
    };
    let delta = super::system::font_zoom_delta(state, *step);
    crate::app::terminal_metrics::apply_terminal_font_zoom(state, id, delta);
}

/// Type `text` into a terminal, and press Enter after it unless told not to.
pub fn handle_terminal_run(state: &mut AppState, action: &WmAction) {
    let WmAction::TerminalRun {
        pane_id,
        terminal,
        text,
        enter,
    } = action
    else {
        return;
    };
    let Some(target) = target_of(state, *pane_id, *terminal) else {
        return;
    };
    let mut bytes = text.as_bytes().to_vec();
    if *enter {
        bytes.push(b'\r');
    }
    let backend = match target {
        Target::Pane(pane) => state.server.backends.get_mut(pane),
        Target::Unowned(id) => state.server.backends.get_mut_by_id(id),
    };
    if let Some(backend) = backend {
        backend.process_input(&bytes);
    }
}

/// End a terminal. A pane's ends by closing its pane, so there is one way a pane's process ends; one
/// no pane owns (an extension's) is ended here, and this is the only thing that ends it.
pub fn handle_terminal_kill(state: &mut AppState, action: &WmAction) {
    let WmAction::TerminalKill { pane_id, terminal } = action else {
        return;
    };
    match target_of(state, *pane_id, *terminal) {
        Some(Target::Pane(pane)) => {
            crate::app::mutations::close_pane_by_id_anywhere(state, pane);
        }
        Some(Target::Unowned(id)) => {
            state.server.backends.kill(id);
            state.mark_full_redraw();
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find(id: TerminalId) -> Option<Target> {
        match id.0 {
            9 => Some(Target::Pane(PaneId(90))),
            7 => Some(Target::Unowned(id)),
            _ => None,
        }
    }

    #[test]
    fn a_terminal_id_wins_then_a_pane_then_the_focused_one() {
        let focused = Some(PaneId(1));
        assert_eq!(
            target(Some(PaneId(2)), Some(9), focused, find),
            Some(Target::Pane(PaneId(90)))
        );
        assert_eq!(
            target(Some(PaneId(2)), None, focused, find),
            Some(Target::Pane(PaneId(2)))
        );
        assert_eq!(
            target(None, None, focused, find),
            Some(Target::Pane(PaneId(1)))
        );
        assert_eq!(target(None, None, None, find), None);
    }

    #[test]
    fn a_terminal_no_pane_owns_is_a_target_by_its_id() {
        assert_eq!(
            target(None, Some(7), Some(PaneId(1)), find),
            Some(Target::Unowned(TerminalId(7)))
        );
    }

    #[test]
    fn an_unknown_terminal_is_no_target_rather_than_the_focused_pane() {
        // Typing into the wrong terminal because an id went stale would be worse than doing nothing.
        assert_eq!(target(None, Some(404), Some(PaneId(1)), find), None);
    }
}
