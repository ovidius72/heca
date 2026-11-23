//! Process I/O belongs to the server; OS clipboard presentation belongs to clients. A terminal bell
//! is attention feedback for its pane, raised here like a finished command.

use super::ServerState;
use heca_core::{backend::BackendAlert, layout::PaneId};

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct BackendPoll {
    pub(crate) has_data: bool,
    pub(crate) terminal_animating: bool,
    pub(crate) clipboard_writes: Vec<String>,
    pub(crate) closing_panes: Vec<PaneId>,
}

impl BackendPoll {
    pub(super) fn has_effects(&self) -> bool {
        !self.clipboard_writes.is_empty()
    }
}

impl ServerState {
    /// Drain process output without reaching any window or OS presentation API.
    pub(crate) fn poll_backends(&mut self) -> BackendPoll {
        let mut result = BackendPoll::default();
        let mut completions = Vec::new();
        let mut bells = Vec::new();
        for (id, backend) in self.backends.iter_mut() {
            result.terminal_animating |= backend.tick_animation();
            result.has_data |= backend.update();
            for alert in backend.take_alerts() {
                match alert {
                    BackendAlert::Bell => bells.push(id),
                    BackendAlert::ProcessCompleted { exit_code } => {
                        completions.push((id, exit_code))
                    }
                }
            }
            result
                .clipboard_writes
                .extend(backend.take_clipboard_writes());
        }
        for id in bells {
            if let Some(pane) = self.backends.pane_of(id) {
                self.feedback.bell(pane);
            }
        }
        for (id, exit_code) in completions {
            if let Some(pane) = self.backends.pane_of(id) {
                self.feedback
                    .completed(pane, exit_code, &self.terminal_settings.quiet_exit_codes);
            }
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
        let poll = BackendPoll {
            has_data: true,
            terminal_animating: true,
            ..Default::default()
        };
        assert!(!poll.has_effects());
    }

    #[test]
    fn presentation_effects_are_not_discarded() {
        assert!(
            BackendPoll {
                clipboard_writes: vec!["text".into()],
                ..Default::default()
            }
            .has_effects()
        );
    }
}
