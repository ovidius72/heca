//! Which terminal the keyboard's search acts on.

use super::*;

impl AppState {
    /// The terminal the keyboard's search acts on: the **selection's** terminal if copy-mode owns
    /// one, else the focused pane's.
    ///
    /// One definition, used by every search entry point — starting a search, stepping matches,
    /// cancelling. They must agree: a search started for one terminal while steps were applied to
    /// another would find nothing to step.
    pub fn search_target(&self) -> Option<crate::chrome::terminal::TerminalId> {
        match self.selection.owner() {
            Some(crate::app::selection_model::SelectionOwner(terminal)) => Some(terminal),
            None => self.server.backends.identity_of(self.focused_pane?),
        }
    }
}
