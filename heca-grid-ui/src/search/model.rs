//! The part a widget embeds: ranks a caller's list with the matcher plus the
//! scope's frecency, and drives history recall from the intents it forwards.

use super::matcher::{fuzzy, MatchCase};
use super::store::SearchStore;
use crate::component::WidgetIntent;
use std::cell::RefCell;
use std::rc::Rc;

/// One ranked item: where it was in the caller's list, and how well it matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ranked {
    /// Index into the slice passed to [`rank`](SearchModel::rank).
    pub index: usize,
    /// Match score **plus** the frecency boost — what the order is by.
    pub score: i32,
    /// The matched character indices, for a caller that marks them.
    pub hits: Vec<usize>,
}

/// What a widget should do about an intent it forwarded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchAction {
    /// Replace the query with this text (and re-filter).
    SetQuery(String),
    /// Not ours — the widget handles it as it would have anyway.
    Ignored,
}

/// The part a widget embeds.
///
/// It holds no text of its own: the widget owns its query field, and passes the current text in.
/// That keeps this usable by a widget with a real `Input`, one with a hand-driven field, and one
/// that has no field at all and filters from elsewhere.
#[derive(Clone)]
pub struct SearchModel {
    scope: String,
    store: Rc<RefCell<SearchStore>>,
    case: MatchCase,
}

impl SearchModel {
    /// A model over `scope` of a host-owned `store`.
    ///
    /// The store is shared and outlives the widget deliberately: a palette is rebuilt every time it
    /// opens, and a memory that died with the widget would remember nothing.
    pub fn new(scope: impl Into<String>, store: Rc<RefCell<SearchStore>>) -> Self {
        Self {
            scope: scope.into(),
            store,
            case: MatchCase::default(),
        }
    }

    /// How the query's case is treated — the user's preference, not the widget's.
    pub fn case(mut self, case: MatchCase) -> Self {
        self.case = case;
        self
    }

    /// The same store and the same preferences, over a **different scope**.
    ///
    /// What a surface with several modes uses: one palette listing actions, panes and workspaces
    /// keeps three separate memories, and switching mode is switching which one it consults.
    pub fn with_scope(&self, scope: impl Into<String>) -> Self {
        Self {
            scope: scope.into(),
            store: self.store.clone(),
            case: self.case,
        }
    }

    /// Which scope this model reads and writes.
    pub fn scope(&self) -> &str {
        &self.scope
    }

    /// A model with a private store — for a surface that wants the behaviour without the memory,
    /// and for tests.
    pub fn detached(scope: impl Into<String>) -> Self {
        Self::new(scope, Rc::new(RefCell::new(SearchStore::new())))
    }

    /// Filter and order `items` for `query`.
    ///
    /// `key` yields each item's `(id, searchable text)`. **The id is optional and belongs to the
    /// caller**: an item without one still matches and still sorts, it just carries no boost — so no
    /// widget is forced to grow an identity field to become searchable.
    ///
    /// Returns one flat list ordered by score. **Grouping is never applied here**: which rows a
    /// caller wants first is its own policy — the command palette leads with the focused component's
    /// actions while the query is empty — and a search module that knew about groups would have to
    /// know about every caller's.
    pub fn rank<T>(
        &self,
        items: &[T],
        query: &str,
        key: impl Fn(&T) -> (Option<&str>, &str),
    ) -> Vec<Ranked> {
        let case_sensitive = self.case.is_sensitive(query);
        let store = self.store.borrow();
        let frecency = store.scope(&self.scope).map(|s| &s.frecency);
        let mut out: Vec<Ranked> = items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                let (id, text) = key(item);
                let m = fuzzy(query, text, case_sensitive)?;
                let boost = match (id, frecency) {
                    (Some(id), Some(f)) => f.boost(id),
                    _ => 0,
                };
                Some(Ranked {
                    index,
                    score: m.score + boost,
                    hits: m.hits,
                })
            })
            .collect();
        // Stable: within equal scores the caller's own order survives.
        out.sort_by_key(|r| std::cmp::Reverse(r.score));
        out
    }

    /// Handle a history intent. **This is the whole of history navigation** — a widget that matches
    /// [`WidgetIntent::MenuHistoryUp`] itself has copied logic that lives here.
    pub fn handle(&mut self, intent: WidgetIntent, current_query: &str) -> SearchAction {
        let mut store = self.store.borrow_mut();
        let history = &mut store.scope_mut(&self.scope).history;
        let recalled = match intent {
            WidgetIntent::MenuHistoryUp => history.older(current_query),
            WidgetIntent::MenuHistoryDown => history.newer(),
            _ => return SearchAction::Ignored,
        };
        match recalled {
            Some(text) => SearchAction::SetQuery(text.to_string()),
            // Nothing to recall: still ours, so the keystroke does not fall through and type.
            None => SearchAction::SetQuery(current_query.to_string()),
        }
    }

    /// An ordinary keystroke: the walk is over, and the field is the user's again.
    pub fn query_changed(&mut self) {
        self.store
            .borrow_mut()
            .scope_mut(&self.scope)
            .history
            .reset();
    }

    /// Something ran: remember the query as typed, and the chosen id if it has one.
    ///
    /// Only on a successful run — an abandoned search is not a search anyone wants back.
    pub fn record_run(&mut self, query: &str, id: Option<&str>) {
        let mut store = self.store.borrow_mut();
        let scope = store.scope_mut(&self.scope);
        scope.history.push(query);
        if let Some(id) = id {
            scope.frecency.record(id);
        }
        // The host watches this to decide when to persist, so a consumer never has to remember to
        // ask for a save — recording *is* asking.
        store.touch();
    }

    /// The shared store, for the host that persists it.
    pub fn store(&self) -> Rc<RefCell<SearchStore>> {
        self.store.clone()
    }
}
