//! Added sound assets use the existing extension startup path, not a plugin loader.

use std::cell::RefCell;

use crate::chrome::StartupQueue;
use super::{Audio, SoundLibrary};

thread_local! {
    static QUEUE: StartupQueue<(String, Audio)> = const { StartupQueue::new() };
    static ADDED: RefCell<Vec<(String, Audio)>> = const { RefCell::new(Vec::new()) };
}

impl crate::Extension {
    /// Register an asset before `heca::run()`. Its configuration name is scoped to this extension.
    /// For example, `extension("demo").sound("ready", Audio::from_bytes(bytes))` supplies
    /// `sound = { source = "asset:demo.ready" }` to ordinary event settings.
    pub fn sound(&self, name: &str, source: Audio) -> &Self {
        let Some(id) = self.full("sound", name) else { return self };
        if let Err((id, _)) = QUEUE.with(|queue| queue.add((id, source))) {
            crate::chrome::warn_author(format!("[heca] sound '{id}' was added after startup and is refused"));
        }
        self
    }
}

pub(super) fn append(mut library: SoundLibrary) -> SoundLibrary {
    let declared = QUEUE.with(StartupQueue::take);
    ADDED.with(|added| {
        let mut added = added.borrow_mut();
        for (id, source) in declared {
            if added.iter().any(|(name, _)| name == &id) {
                crate::chrome::warn_author(format!("[heca] duplicate sound asset '{id}' is refused"));
            } else {
                added.push((id, source));
            }
        }
        for (id, source) in added.iter() { library = library.sound(id, source.clone()); }
        library
    })
}

#[cfg(test)]
mod tests;
