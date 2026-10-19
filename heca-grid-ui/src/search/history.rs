//! The queries a scope has been searched with, newest last, with a cursor for
//! walking them — the up/down recall every shell prompt has.

use super::DEFAULT_HISTORY_CAP;

/// The queries a scope has been searched with, newest last, with a cursor for walking them.
#[derive(Debug, Clone)]
pub struct History {
    cap: usize,
    entries: Vec<String>,
    /// Where the walk is. `None` means "not walking" — the field holds what the user typed.
    cursor: Option<usize>,
    /// What the user had typed before the walk started, restored on the way back down.
    draft: Option<String>,
}

impl Default for History {
    fn default() -> Self {
        Self {
            cap: DEFAULT_HISTORY_CAP,
            entries: Vec::new(),
            cursor: None,
            draft: None,
        }
    }
}

impl History {
    /// An empty history.
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty history keeping at most `cap` queries.
    pub fn with_cap(cap: usize) -> Self {
        Self {
            cap,
            ..Self::default()
        }
    }

    /// How many queries this history keeps.
    pub fn cap(&self) -> usize {
        self.cap
    }

    /// Remember a query. Ignores an empty one and a repeat of the newest — retyping the same search
    /// must not fill the ring.
    pub fn push(&mut self, query: &str) {
        self.reset();
        if query.is_empty() || self.entries.last().map(String::as_str) == Some(query) {
            return;
        }
        self.entries.push(query.to_string());
        if self.entries.len() > self.cap {
            self.entries.remove(0);
        }
    }

    /// Step to an older query, stashing `current` if the walk is starting.
    ///
    /// Stops at the oldest rather than wrapping: a history that loops hides from the user that they
    /// have seen everything.
    pub fn older(&mut self, current: &str) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        let next = match self.cursor {
            None => {
                self.draft = Some(current.to_string());
                self.entries.len() - 1
            }
            Some(0) => 0,
            Some(i) => i - 1,
        };
        self.cursor = Some(next);
        Some(&self.entries[next])
    }

    /// Step to a newer query — and past the newest, back to **the draft the walk interrupted**.
    /// Every shell prompt does that, and its absence is felt immediately: the field would be
    /// stranded on the last history entry with what you were typing gone.
    pub fn newer(&mut self) -> Option<&str> {
        let i = self.cursor?;
        if i + 1 < self.entries.len() {
            self.cursor = Some(i + 1);
            return Some(&self.entries[i + 1]);
        }
        self.cursor = None;
        Some(self.draft.as_deref().unwrap_or(""))
    }

    /// Leave the walk — any ordinary keystroke does.
    pub fn reset(&mut self) {
        self.cursor = None;
        self.draft = None;
    }

    /// The remembered queries, oldest first — for the host that persists this.
    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    /// Rebuild from persisted state, oldest first.
    pub fn restore(cap: usize, entries: impl IntoIterator<Item = String>) -> Self {
        let mut entries: Vec<String> = entries.into_iter().collect();
        let overflow = entries.len().saturating_sub(cap);
        entries.drain(..overflow);
        Self {
            cap,
            entries,
            cursor: None,
            draft: None,
        }
    }
}
