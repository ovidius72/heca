//! **Which terminal an action means** — the one rule every terminal action names its target by,
//! whether a window or the server runs it.

use heca_core::layout::PaneId;

use super::BackendStore;
use crate::chrome::panes::terminal::TerminalId;

/// A pane's terminal, or one no pane owns (an extension's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TerminalTarget {
    Pane(PaneId),
    Unowned(TerminalId),
}

impl BackendStore {
    /// Resolve a numeric identity or an exact qualified owner name, never a display title.
    pub(crate) fn resolve_reference(&self, reference: &str) -> Option<TerminalId> {
        match reference.parse::<u64>() {
            Ok(id) => self.is_issued(TerminalId(id)).then_some(TerminalId(id)),
            Err(_) => self.find_owner(&super::TerminalOwner::Named(reference.to_owned())),
        }
    }

    /// **Which terminal an action means.** A `terminal` id wins (it is what a script that started
    /// one holds), then an explicit `pane_id`, then the pane `focused` — the one with the keyboard.
    /// An id that names no terminal is no target, never the focused pane.
    pub(crate) fn target(
        &self,
        pane_id: Option<PaneId>,
        terminal: Option<u64>,
        focused: Option<PaneId>,
    ) -> Option<TerminalTarget> {
        target(pane_id, terminal, focused, |id| match self.pane_of(id) {
            Some(pane) => Some(TerminalTarget::Pane(pane)),
            None => self.get_by_id(id).map(|_| TerminalTarget::Unowned(id)),
        })
    }

    /// The terminal itself, whoever owns it.
    pub(crate) fn terminal_of(&self, target: TerminalTarget) -> Option<TerminalId> {
        match target {
            TerminalTarget::Pane(pane) => self.identity_of(pane),
            TerminalTarget::Unowned(id) => Some(id),
        }
    }
}

fn target(
    pane_id: Option<PaneId>,
    terminal: Option<u64>,
    focused: Option<PaneId>,
    find: impl Fn(TerminalId) -> Option<TerminalTarget>,
) -> Option<TerminalTarget> {
    match terminal {
        Some(id) => find(TerminalId(id)),
        None => pane_id.or(focused).map(TerminalTarget::Pane),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_use_exact_owner_names_or_issued_ids_never_custom_titles() {
        let mut store = BackendStore::new();
        let first = store.id_for(super::super::TerminalOwner::Named("demo.shell".into()));
        let other = store.id_for(super::super::TerminalOwner::Named("other.shell".into()));
        store.set_custom_title(first, Some("Renamed".into()));
        for name in ["demo.shell", &first.0.to_string()] {
            assert_eq!(store.resolve_reference(name), Some(first));
        }
        assert_eq!(store.resolve_reference("other.shell"), Some(other));
        for missing in ["shell", "Renamed", "DEMO.shell", "missing.shell", "9999", ""] {
            assert_eq!(store.resolve_reference(missing), None, "{missing}");
        }
    }

    fn find(id: TerminalId) -> Option<TerminalTarget> {
        match id.0 {
            9 => Some(TerminalTarget::Pane(PaneId(90))),
            7 => Some(TerminalTarget::Unowned(id)),
            _ => None,
        }
    }

    #[test]
    fn a_terminal_id_wins_then_a_pane_then_the_focused_one() {
        let focused = Some(PaneId(1));
        assert_eq!(
            target(Some(PaneId(2)), Some(9), focused, find),
            Some(TerminalTarget::Pane(PaneId(90)))
        );
        assert_eq!(
            target(Some(PaneId(2)), None, focused, find),
            Some(TerminalTarget::Pane(PaneId(2)))
        );
        assert_eq!(
            target(None, None, focused, find),
            Some(TerminalTarget::Pane(PaneId(1)))
        );
        assert_eq!(target(None, None, None, find), None);
    }

    #[test]
    fn a_terminal_no_pane_owns_is_a_target_by_its_id() {
        assert_eq!(
            target(None, Some(7), Some(PaneId(1)), find),
            Some(TerminalTarget::Unowned(TerminalId(7)))
        );
    }

    #[test]
    fn an_unknown_terminal_is_no_target_rather_than_the_focused_pane() {
        // Typing into the wrong terminal because an id went stale would be worse than doing nothing.
        assert_eq!(target(None, Some(404), Some(PaneId(1)), find), None);
    }
}
