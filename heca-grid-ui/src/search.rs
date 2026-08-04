//! Search as a capability a widget **embeds** — matching, ranking by use, and query history.
//!
//! The shape is [`ScrollRegion`](crate::widgets::ScrollRegion)'s. That widget owns the paging, the
//! clamp and the axis-decline, and anything needing to scroll *nests one* rather than reimplementing
//! it. A widget that needs to be searchable embeds a [`SearchModel`] and reimplements nothing: it
//! forwards its list to [`rank`](SearchModel::rank), forwards the history intents to
//! [`handle`](SearchModel::handle), and tells it when something ran.
//!
//! **No filesystem, no clock, and no widget type is named here.** That is what keeps it testable
//! without a window and reusable by whatever searches next — the `>` / `@` / `:` modes a palette
//! grows, a filterable `Select`, a menu. Persisting the store is the *app's* job: a UI library has
//! no business knowing where an install keeps its state, and a widget test would otherwise need a
//! temp directory to run at all.

use crate::component::WidgetIntent;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// How much a match's **use count** is worth: `COUNT_WEIGHT × log2(1 + count)`, so the first few
/// uses separate an entry and the hundredth barely moves it.
const COUNT_WEIGHT: i32 = 4;
/// How much a match's **recency** is worth, decaying as `RECENCY_WEIGHT / (1 + age)`. Smaller than
/// the count term on purpose: how often you reach for something says more than whether it was the
/// very last thing you reached for.
const RECENCY_WEIGHT: i32 = 6;
/// The ceiling on the whole frecency term.
///
/// **The cap is the design decision.** A textual score is small — a start-of-word hit is +8 — so an
/// uncapped frecency term would let an entry the user picks often outrank one the query actually
/// spells better, and typing would stop feeling like it did anything. Frecency decides ties and
/// near-ties; a clearly better match still wins.
///
/// It must also sit **above** where ordinary use lands, or it stops being a ceiling and becomes the
/// answer: the first calibration here capped at 18 while five-uses-recently and once-just-now both
/// summed past it, so the two scored identically and the ranking said nothing. Everyday values land
/// around 4–14; this is the wall for the outliers.
const BOOST_MAX: i32 = 20;

/// How many past queries a scope remembers.
const HISTORY_CAP: usize = 50;
/// How many ids a scope's usage table keeps before pruning the least valuable.
const FRECENCY_CAP: usize = 500;

/// A match: how well the query fits, and which characters it landed on.
///
/// The hits are what a [`Label`](crate::widgets::Label) draws as marks, which is why they are
/// character indices into the source text and not byte offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    /// Higher is better.
    pub score: i32,
    /// Indices of the matched characters, in order.
    pub hits: Vec<usize>,
}

/// Should this query match case-sensitively? Only when it carries an uppercase character.
///
/// Named rather than left inline so the second search surface cannot spell the rule differently.
pub fn smart_case(query: &str) -> bool {
    query.chars().any(|c| c.is_uppercase())
}

/// Fuzzy subsequence match: every query character must appear in `text`, in order.
///
/// Scores consecutive runs (+5) and start-of-word hits (+8), and prefers tighter matches. An empty
/// query matches everything with a score of zero — which is what lets frecency alone order a list
/// nobody has typed into yet.
pub fn fuzzy(query: &str, text: &str, case_sensitive: bool) -> Option<Match> {
    let q: Vec<char> = query.chars().collect();
    if q.is_empty() {
        return Some(Match { score: 0, hits: Vec::new() });
    }
    let t: Vec<char> = text.chars().collect();
    let norm = |c: char| {
        if case_sensitive {
            c
        } else {
            c.to_ascii_lowercase()
        }
    };
    let mut qi = 0;
    let mut hits = Vec::with_capacity(q.len());
    let mut score = 0i32;
    let mut prev: Option<usize> = None;
    for (ti, &tc) in t.iter().enumerate() {
        if norm(tc) == norm(q[qi]) {
            score += 1;
            if prev == Some(ti.wrapping_sub(1)) {
                score += 5; // consecutive run
            }
            if ti == 0 || !t[ti - 1].is_alphanumeric() {
                score += 8; // start of a word
            }
            hits.push(ti);
            prev = Some(ti);
            qi += 1;
            if qi == q.len() {
                // Prefer shorter / tighter matches.
                score -= (t.len() as i32 - q.len() as i32) / 4;
                return Some(Match { score, hits });
            }
        }
    }
    None
}

/// One entry's usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Use {
    count: u32,
    last_seq: u64,
}

/// How often and how recently each entry has been chosen.
///
/// **Recency is distance in the use-sequence, never a timestamp.** No clock in a UI library means no
/// clock skew, no timezone and no sleeping in a test — a test asserts an exact number. The sequence
/// is part of what gets persisted, or every restart would reset "everything is equally old".
#[derive(Debug, Clone, Default)]
pub struct Frecency {
    uses: HashMap<String, Use>,
    seq: u64,
}

impl Frecency {
    /// An empty table — the first-run state, in which [`boost`](Self::boost) is always 0 and a list's
    /// order is exactly what the matcher alone would give.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that `id` was chosen.
    pub fn record(&mut self, id: &str) {
        self.seq += 1;
        let seq = self.seq;
        let entry = self.uses.entry(id.to_string()).or_insert(Use { count: 0, last_seq: seq });
        entry.count = entry.count.saturating_add(1);
        entry.last_seq = seq;
        self.prune();
    }

    /// What this id adds to a match score. 0 for an id never chosen.
    pub fn boost(&self, id: &str) -> i32 {
        let Some(u) = self.uses.get(id) else {
            return 0;
        };
        // log2(1 + count), integer: 1 use -> 1, 3 -> 2, 7 -> 3, 15 -> 4 …
        let count_term = COUNT_WEIGHT * (u32::BITS - (u.count + 1).leading_zeros() - 1) as i32;
        let age = self.seq.saturating_sub(u.last_seq);
        let recency_term = RECENCY_WEIGHT / (1 + age.min(i32::MAX as u64) as i32);
        (count_term + recency_term).min(BOOST_MAX)
    }

    /// Every entry, for the host that persists this.
    pub fn entries(&self) -> impl Iterator<Item = (&str, u32, u64)> {
        self.uses.iter().map(|(id, u)| (id.as_str(), u.count, u.last_seq))
    }

    /// The current use-sequence — persisted with the entries, or recency resets on restart.
    pub fn seq(&self) -> u64 {
        self.seq
    }

    /// Rebuild from persisted state.
    pub fn restore(seq: u64, entries: impl IntoIterator<Item = (String, u32, u64)>) -> Self {
        let mut out = Self {
            uses: entries
                .into_iter()
                .map(|(id, count, last_seq)| (id, Use { count, last_seq }))
                .collect(),
            seq,
        };
        out.prune();
        out
    }

    /// Keep the table bounded: a long-lived install must not grow without limit. The least valuable
    /// entries go, which is the same judgement `boost` makes.
    fn prune(&mut self) {
        if self.uses.len() <= FRECENCY_CAP {
            return;
        }
        let mut ranked: Vec<(String, i32)> =
            self.uses.keys().map(|id| (id.clone(), self.boost(id))).collect();
        ranked.sort_by_key(|(_, boost)| *boost);
        for (id, _) in ranked.into_iter().take(self.uses.len() - FRECENCY_CAP) {
            self.uses.remove(&id);
        }
    }
}

/// The queries a scope has been searched with, newest last, with a cursor for walking them.
#[derive(Debug, Clone, Default)]
pub struct History {
    entries: Vec<String>,
    /// Where the walk is. `None` means "not walking" — the field holds what the user typed.
    cursor: Option<usize>,
    /// What the user had typed before the walk started, restored on the way back down.
    draft: Option<String>,
}

impl History {
    /// An empty history.
    pub fn new() -> Self {
        Self::default()
    }

    /// Remember a query. Ignores an empty one and a repeat of the newest — retyping the same search
    /// must not fill the ring.
    pub fn push(&mut self, query: &str) {
        self.reset();
        if query.is_empty() || self.entries.last().map(String::as_str) == Some(query) {
            return;
        }
        self.entries.push(query.to_string());
        if self.entries.len() > HISTORY_CAP {
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
    pub fn restore(entries: impl IntoIterator<Item = String>) -> Self {
        let mut entries: Vec<String> = entries.into_iter().collect();
        let overflow = entries.len().saturating_sub(HISTORY_CAP);
        entries.drain(..overflow);
        Self { entries, cursor: None, draft: None }
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
}

impl SearchStore {
    /// An empty store — the first-run state. Everything ranks exactly as the matcher alone would.
    pub fn new() -> Self {
        Self::default()
    }

    /// One scope's memory, created empty on first use.
    pub fn scope_mut(&mut self, scope: &str) -> &mut Scope {
        self.scopes.entry(scope.to_string()).or_default()
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
}

impl SearchModel {
    /// A model over `scope` of a host-owned `store`.
    ///
    /// The store is shared and outlives the widget deliberately: a palette is rebuilt every time it
    /// opens, and a memory that died with the widget would remember nothing.
    pub fn new(scope: impl Into<String>, store: Rc<RefCell<SearchStore>>) -> Self {
        Self { scope: scope.into(), store }
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
        let case_sensitive = smart_case(query);
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
                Some(Ranked { index, score: m.score + boost, hits: m.hits })
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
        self.store.borrow_mut().scope_mut(&self.scope).history.reset();
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
    }

    /// The shared store, for the host that persists it.
    pub fn store(&self) -> Rc<RefCell<SearchStore>> {
        self.store.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_matches_subsequence_and_scores_consecutive_higher() {
        assert!(fuzzy("xyz", "abc", false).is_none(), "not a subsequence");
        assert!(fuzzy("ace", "abcde", false).is_some());
        assert_eq!(fuzzy("ce", "abcde", false).unwrap().hits, vec![2, 4]);
        let consecutive = fuzzy("ab", "abxx", false).unwrap().score;
        let scattered = fuzzy("ab", "axbx", false).unwrap().score;
        assert!(consecutive > scattered, "{consecutive} vs {scattered}");
        // An empty query matches everything at zero — which is what lets frecency alone order a
        // list nobody has typed into.
        assert_eq!(fuzzy("", "anything", false).unwrap().score, 0);
    }

    #[test]
    fn smart_case_is_one_rule() {
        assert!(!smart_case("git"));
        assert!(smart_case("Git"));
        assert!(fuzzy("git", "Git Push", smart_case("git")).is_some());
        assert!(fuzzy("G", "git pull", smart_case("G")).is_none());
    }

    /// Chosen more often outranks chosen once; used recently outranks used long ago.
    #[test]
    fn frecency_counts_uses_and_how_recent_they_were() {
        let mut f = Frecency::new();
        for _ in 0..5 {
            f.record("often");
        }
        f.record("once");
        assert!(
            f.boost("often") > f.boost("once"),
            "{} vs {}",
            f.boost("often"),
            f.boost("once"),
        );
        assert_eq!(f.boost("never"), 0, "an id never chosen adds nothing");

        // Same count, different age.
        let mut g = Frecency::new();
        g.record("old");
        for i in 0..20 {
            g.record(&format!("filler{i}"));
        }
        g.record("new");
        assert!(g.boost("new") > g.boost("old"), "recency separates equal counts");
    }

    /// **The cap is the point.** A much-used entry must not overtake a clearly better textual match,
    /// or typing stops meaning anything.
    #[test]
    fn frecency_decides_ties_but_never_beats_a_better_match() {
        let store = Rc::new(RefCell::new(SearchStore::new()));
        let mut model = SearchModel::new("command", store);
        for _ in 0..50 {
            model.record_run("", Some("much_used"));
        }
        let items = [("much_used", "Zoom Out Everything"), ("close", "Close Pane")];
        let ranked = model.rank(&items, "close", |i| (Some(i.0), i.1));
        assert_eq!(
            items[ranked[0].index].0, "close",
            "the query spells `Close Pane`, so it leads however often the other was chosen",
        );

        // With nothing typed, the boost is all there is — and now it leads.
        let empty = model.rank(&items, "", |i| (Some(i.0), i.1));
        assert_eq!(items[empty[0].index].0, "much_used");
    }

    /// An empty store must reproduce the matcher's own order exactly — first-run behaviour cannot
    /// differ from no-frecency behaviour.
    #[test]
    fn an_empty_store_changes_no_ordering() {
        let items = [("a", "Close Pane"), ("b", "Close Window")];
        let with_ids = SearchModel::detached("command").rank(&items, "close", |i| (Some(i.0), i.1));
        let without = SearchModel::detached("command").rank(&items, "close", |i| (None, i.1));
        assert_eq!(with_ids, without);
    }

    #[test]
    fn history_dedupes_and_stops_at_the_oldest() {
        let mut h = History::new();
        h.push("");
        h.push("close");
        h.push("close");
        h.push("split");
        assert_eq!(h.entries(), ["close", "split"], "empty and repeats are not remembered");

        assert_eq!(h.older(""), Some("split"));
        assert_eq!(h.older(""), Some("close"));
        assert_eq!(h.older(""), Some("close"), "the oldest is a wall, not a loop");
    }

    /// Walking back down past the newest restores **what the user was typing**, not the last entry.
    #[test]
    fn history_gives_the_draft_back() {
        let mut h = History::new();
        h.push("close");
        h.push("split");
        assert_eq!(h.older("dra"), Some("split"));
        assert_eq!(h.older("dra"), Some("close"));
        assert_eq!(h.newer(), Some("split"));
        assert_eq!(h.newer(), Some("dra"), "the interrupted draft comes back");
        assert_eq!(h.newer(), None, "and the walk is over");
    }

    /// The full walk driven **by intents alone, with no widget in the test**. If this needed one,
    /// the seam would be in the wrong place.
    #[test]
    fn the_intents_drive_the_whole_walk() {
        let mut model = SearchModel::detached("command");
        model.record_run("close", Some("close"));
        model.record_run("split", Some("split"));

        let up = |m: &mut SearchModel, q: &str| match m.handle(WidgetIntent::MenuHistoryUp, q) {
            SearchAction::SetQuery(t) => t,
            other => panic!("expected a query, got {other:?}"),
        };
        let down = |m: &mut SearchModel| match m.handle(WidgetIntent::MenuHistoryDown, "") {
            SearchAction::SetQuery(t) => t,
            other => panic!("expected a query, got {other:?}"),
        };
        assert_eq!(up(&mut model, "typed"), "split");
        assert_eq!(up(&mut model, "typed"), "close");
        assert_eq!(down(&mut model), "split");
        assert_eq!(down(&mut model), "typed", "back to the draft");

        // Anything else is not ours.
        assert_eq!(
            model.handle(WidgetIntent::MenuDown, ""),
            SearchAction::Ignored,
        );
    }

    /// A scope is a separate memory — that is what makes `>` / `@` / `:` data rather than a reshape.
    #[test]
    fn scopes_do_not_share_a_history() {
        let store = Rc::new(RefCell::new(SearchStore::new()));
        let mut commands = SearchModel::new("command", store.clone());
        let mut symbols = SearchModel::new("symbol", store);
        commands.record_run("close", None);
        assert_eq!(
            symbols.handle(WidgetIntent::MenuHistoryUp, "q"),
            SearchAction::SetQuery("q".to_string()),
            "the symbol scope has no history of its own yet",
        );
    }

    /// An abandoned search records nothing, and a run without an id still records the query.
    #[test]
    fn only_a_run_is_remembered() {
        let mut model = SearchModel::detached("command");
        model.query_changed();
        assert_eq!(
            model.handle(WidgetIntent::MenuHistoryUp, "typed"),
            SearchAction::SetQuery("typed".to_string()),
            "nothing was run, so there is nothing to recall",
        );
        model.record_run("filter", None);
        assert_eq!(
            model.handle(WidgetIntent::MenuHistoryUp, "typed"),
            SearchAction::SetQuery("filter".to_string()),
        );
    }
}
