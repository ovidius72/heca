//! User labels belong to the terminal owner, never its process or launch specification.

use super::{BackendStore, TerminalId};

impl BackendStore {
    pub(crate) fn custom_title(&self, id: TerminalId) -> Option<&str> {
        self.custom_titles.get(&id).map(String::as_str)
    }

    pub(crate) fn set_custom_title(&mut self, id: TerminalId, title: Option<String>) -> bool {
        if !self.is_issued(id) || self.custom_titles.get(&id) == title.as_ref() {
            return false;
        }
        match title {
            Some(title) => { self.custom_titles.insert(id, title); }
            None => { self.custom_titles.remove(&id); }
        }
        true
    }
}
