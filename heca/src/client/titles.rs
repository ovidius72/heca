//! Apply shared title metadata from changes and replies through one receive path.

use super::{ClientState, Update};
use crate::server::Change;

impl ClientState {
    pub(super) fn receive_titles(&mut self, update: &Update) {
        for change in update.changes.iter().chain(update.answers.iter().flat_map(|answer| &answer.changes)) {
            if let Change::TerminalTitleChanged { owner, title } = change
                && let Some(id) = self.backends.find_owner(owner)
            {
                self.backends.set_custom_title(id, title.clone());
            }
        }
    }
}
