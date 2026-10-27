//! Process I/O belongs to the server; OS clipboard and bell presentation belong to clients.

use heca_core::{backend::BackendAlert, layout::PaneId};
use super::ServerState;

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct BackendPoll {
    pub(crate) has_data: bool,
    pub(crate) bell_any: bool,
    pub(crate) terminal_animating: bool,
    pub(crate) clipboard_writes: Vec<String>,
    pub(crate) closing_panes: Vec<PaneId>,
}

impl BackendPoll {
    pub(super) fn has_effects(&self) -> bool {
        self.bell_any || !self.clipboard_writes.is_empty()
    }
}

impl ServerState {
    /// Drain process output without reaching any window or OS presentation API.
    pub(crate) fn poll_backends(&mut self) -> BackendPoll {
        let mut result = BackendPoll::default();
        for backend in self.backends.values_mut() {
            result.terminal_animating |= backend.tick_animation();
            result.has_data |= backend.update();
            result.bell_any |= backend.take_alerts().into_iter()
                .any(|alert| matches!(alert, BackendAlert::Bell));
            result.clipboard_writes.extend(backend.take_clipboard_writes());
        }
        result.closing_panes = self.backends.pane_ids_to_close();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::BackendPoll;

    #[test]
    fn output_is_carried_by_replaceable_terminal_frames() {
        let poll = BackendPoll { has_data: true, terminal_animating: true, ..Default::default() };
        assert!(!poll.has_effects());
    }

    #[test]
    fn presentation_effects_are_not_discarded() {
        assert!(BackendPoll { bell_any: true, ..Default::default() }.has_effects());
        assert!(BackendPoll { clipboard_writes: vec!["text".into()], ..Default::default() }.has_effects());
    }
}
