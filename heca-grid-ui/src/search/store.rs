//! Every scope's memory in one place — history and ranking per scope — so a host
//! persists a single subject and a palette adds a mode as data, not a reshape.

use super::frecency::Frecency;
use super::history::History;
use std::collections::HashMap;

/// How many past queries a scope remembers, unless the host says otherwise.
pub const DEFAULT_HISTORY_CAP: usize = 50;
/// How many ids a scope's usage table keeps before pruning the least valuable, unless the host says
/// otherwise.
pub const DEFAULT_USAGE_CAP: usize = 500;

/// How much a scope remembers. The host's, because it is the one with a config file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caps {
    /// Past queries kept per scope.
    pub history: usize,
    /// Usage entries kept per scope.
    pub usage: usize,
}

impl Default for Caps {
    fn default() -> Self {
        Self {
            history: DEFAULT_HISTORY_CAP,
            usage: DEFAULT_USAGE_CAP,
        }
    }
}

/// One search scope's memory: what has been searched, and what has been chosen.
#[derive(Debug, Clone, Default)]
pub struct Scope {
    /// Past queries.
    pub history: History,
    /// How often and how recently each entry was chosen.
    pub frecency: Frecency,
}

/// Every scope's memory — the whole snapshot a host persists, so serialisation has one subject.
///
/// **Scopes exist from the first line written, with only `"command"` in use.** Retrofitting a key
/// into a persisted file means migrating it, and a palette that grows `>` / `@` / `:` modes must be
/// able to add one as *data*, not as a reshape.
#[derive(Debug, Clone, Default)]
pub struct SearchStore {
    scopes: HashMap<String, Scope>,
    caps: Caps,
    revision: u64,
}

impl SearchStore {
    /// An empty store — the first-run state. Everything ranks exactly as the matcher alone would.
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty store remembering `caps` much per scope.
    pub fn with_caps(caps: Caps) -> Self {
        Self {
            caps,
            ..Self::default()
        }
    }

    /// How much this store remembers per scope.
    pub fn caps(&self) -> Caps {
        self.caps
    }

    /// One scope's memory, created empty on first use — with this store's caps.
    pub fn scope_mut(&mut self, scope: &str) -> &mut Scope {
        let caps = self.caps;
        self.scopes
            .entry(scope.to_string())
            .or_insert_with(|| Scope {
                history: History::with_cap(caps.history),
                frecency: Frecency::with_cap(caps.usage),
            })
    }

    /// Bumped every time something is recorded.
    ///
    /// **This is what makes persistence impossible to forget.** A host saves when this changes,
    /// rather than every consumer remembering to ask — a second search surface that forgot would
    /// silently stop being remembered, and nothing would report it.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Note that something was recorded.
    pub fn touch(&mut self) {
        self.revision += 1;
    }

    /// Forget past queries — one scope, or every scope when `scope` is `None`.
    ///
    /// Separate from [`clear_ranking`](Self::clear_ranking) because they are different intentions:
    /// forgetting what you typed is not forgetting what you use.
    ///
    /// Returns whether anything was there to forget.
    pub fn clear_history(&mut self, scope: Option<&str>) -> bool {
        self.clear_with(scope, |s| {
            let had = !s.history.entries().is_empty();
            s.history = History::with_cap(s.history.cap());
            had
        })
    }

    /// Forget the usage that ranks a list — one scope, or every scope when `scope` is `None`.
    pub fn clear_ranking(&mut self, scope: Option<&str>) -> bool {
        self.clear_with(scope, |s| {
            let had = s.frecency.entries().next().is_some();
            s.frecency = Frecency::with_cap(s.frecency.cap());
            had
        })
    }

    fn clear_with(&mut self, scope: Option<&str>, mut f: impl FnMut(&mut Scope) -> bool) -> bool {
        let mut changed = false;
        match scope {
            Some(name) => {
                if let Some(s) = self.scopes.get_mut(name) {
                    changed = f(s);
                }
            }
            None => {
                for s in self.scopes.values_mut() {
                    changed |= f(s);
                }
            }
        }
        if changed {
            self.touch();
        }
        changed
    }

    /// One scope's memory, if it has any yet.
    pub fn scope(&self, scope: &str) -> Option<&Scope> {
        self.scopes.get(scope)
    }

    /// Every scope, for the host that persists this.
    pub fn scopes(&self) -> impl Iterator<Item = (&str, &Scope)> {
        self.scopes.iter().map(|(name, s)| (name.as_str(), s))
    }

    /// Insert a scope's restored memory.
    pub fn insert(&mut self, scope: impl Into<String>, state: Scope) {
        self.scopes.insert(scope.into(), state);
    }
}
