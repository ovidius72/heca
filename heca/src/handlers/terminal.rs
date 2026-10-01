//! **Terminal actions** — typing a line into a running terminal, and ending one. The two ways
//! anything outside the terminal (a key, the palette, RPC, heca-pro) reaches a terminal process.

use crate::app_state::AppState;
use crate::chrome::terminal::TerminalId;
use crate::input::WmAction;
use heca_core::layout::PaneId;

/// **Which pane's terminal an action means.** A `terminal` id wins (it is what a script that started
/// one holds), then an explicit `pane_id`, then the pane that has the keyboard. One rule, so every
/// terminal action reads its target the same way.
fn target(
    pane_id: Option<PaneId>,
    terminal: Option<u64>,
    focused: Option<PaneId>,
    pane_of: impl Fn(TerminalId) -> Option<PaneId>,
) -> Option<PaneId> {
    match terminal {
        Some(id) => pane_of(TerminalId(id)),
        None => pane_id.or(focused),
    }
}

fn target_of(state: &AppState, pane_id: Option<PaneId>, terminal: Option<u64>) -> Option<PaneId> {
    target(pane_id, terminal, state.focused_pane, |id| {
        state.backends.pane_of(id)
    })
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
    let Some(pane) = target_of(state, *pane_id, *terminal) else {
        return;
    };
    let mut bytes = text.as_bytes().to_vec();
    if *enter {
        bytes.push(b'\r');
    }
    if let Some(backend) = state.backends.get_mut(pane) {
        backend.process_input(&bytes);
    }
}

/// End a terminal — which is what closing its pane does, so there is one way a process ends.
pub fn handle_terminal_kill(state: &mut AppState, action: &WmAction) {
    let WmAction::TerminalKill { pane_id, terminal } = action else {
        return;
    };
    if let Some(pane) = target_of(state, *pane_id, *terminal) {
        crate::app::mutations::close_pane_by_id_anywhere(state, pane);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner(id: TerminalId) -> Option<PaneId> {
        (id.0 == 9).then_some(PaneId(90))
    }

    #[test]
    fn a_terminal_id_wins_then_a_pane_then_the_focused_one() {
        let focused = Some(PaneId(1));
        assert_eq!(
            target(Some(PaneId(2)), Some(9), focused, owner),
            Some(PaneId(90))
        );
        assert_eq!(
            target(Some(PaneId(2)), None, focused, owner),
            Some(PaneId(2))
        );
        assert_eq!(target(None, None, focused, owner), Some(PaneId(1)));
        assert_eq!(target(None, None, None, owner), None);
    }

    #[test]
    fn an_unknown_terminal_is_no_target_rather_than_the_focused_pane() {
        // Typing into the wrong terminal because an id went stale would be worse than doing nothing.
        assert_eq!(target(None, Some(404), Some(PaneId(1)), owner), None);
    }
}
